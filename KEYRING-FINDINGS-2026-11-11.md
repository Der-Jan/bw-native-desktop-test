# Secret-Service-/KWallet-Diagnose und Debian-Paketanforderungen

Stand: 2026-11-11. Übergabe zur weiteren Entwicklung von bitwarden-desktop-next.

## Umgebung und Quellenstand

- Webtop: `lscr.io/linuxserver/webtop:ubuntu-kde-69ec695c-ls147`, Container `webtop2`, Ubuntu 24.04.
- KWallet 5: `libkf5wallet-bin`, `libkf5wallet5`, `libkf5wallet-data`, Version `5.115.0-0ubuntu3`.
- Installierter Client: `0.1.0+git8cf10baca6f1-1ubuntu24.04`.
- Paket-Homepage: https://github.com/Der-Jan/bw-native-desktop-test
- Verfügbarer Repository-Stand: `62509fb` (dev). Der installierte Build-Commit `8cf10baca6f1` war im separat geklonten Repository nicht auflösbar. Vor einer Korrektur mit dem Entwicklungsstand auf dem anderen PC abgleichen.
- Client verwendet `keyring 3.6.3`, Features unter anderem `sync-secret-service`, `crypto-rust`; Backend `dbus-secret-service 4.1.0`.
- Relevanter Client-Code: `crates/desktop/src/services/accounts/storage.rs`, `OsKeyStore::{put,get,delete}`.

## Nachgewiesene Fehler und Maßnahmen

### 1. GPG-Passwortdialog ohne geeignetes Terminal

Anfangs war nur `pinentry-curses` installiert. Beim Öffnen der GPG-geschützten Wallet trat ein Fehler vom Typ „Inappropriate ioctl for device“ auf. `GPG_TTY` und `gpg-connect-agent updatestartuptty /bye` halfen hier nicht.

`pinentry-qt` wurde installiert und automatisch über die alternatives-Verwaltung als `/usr/bin/pinentry` ausgewählt. Der Nutzer bestätigte danach, dass der Passwortdialog funktioniert.

### 2. Native KWallet- und Secret-Service-Sperrstatus unterschieden sich

Die Standard-Collection lautet `/org/freedesktop/secrets/collection/Chrome_20Wallet`, Label `Chrome Wallet`; Anbieter ist `kwalletd5`.

Nach nativem Öffnen meldete `org.kde.KWallet.isOpen("Chrome Wallet")` bereits `true`, die Secret-Service-Property `org.freedesktop.Secret.Collection.Locked` aber noch `true`. Ein Aufruf von `org.freedesktop.Secret.Service.Unlock` mit anschließendem `org.freedesktop.Secret.Prompt.Prompt` setzte den Secret-Service-Status auf `false`.

Das allein behob den Clientfehler nicht. Ein geöffneter Pinentry-Dialog bzw. eine native offene Wallet beweist nicht, dass der vollständige Secret-Service-Schreibpfad funktioniert.

### 3. Fehlendes QCA-OpenSSL-Plugin

Ein separater Rust-Test mit derselben Keyring-Version und denselben Linux-Backend-/Crypto-Features lieferte beim Speichern künstlicher Werte:

```text
PlatformFailure(Dbus(D-Bus error: createDLGroup failed: maybe libqca-ossl is missing (org.freedesktop.DBus.Error.InvalidArgs)))
```

`libqca-qt5-2` war installiert, `libqca-qt5-2-plugins` fehlte. Nach Installation von `libqca-qt5-2-plugins` und Neustart von `kwalletd5` funktionierte der verschlüsselte Secret-Service-Schreibpfad.

Der Neustart war erforderlich: Der bereits laufende Dienst meldete auch nach Paketinstallation zunächst denselben Fehler. In diesem Image existierte nur die D-Bus-Startregistrierung `org.kde.kwalletd5`, keine eigene `org.freedesktop.secrets.service`. Nach Beenden musste KWallet explizit über `StartServiceByName("org.kde.kwalletd5", 0)` wieder gestartet werden.

### 4. Binärschlüssel werden bei diesem Backend verändert

Nach Behebung des QCA-Fehlers wurde mit dem Testprogramm ein vollständiger Schreib-/Lese-/Löschzyklus ausgeführt:

```text
text: readback matches = Ok(true); cleanup = Ok(())
text: Ok(())
binary: readback matches = Ok(false); cleanup = Ok(())
binary: Ok(())
```

Text-Testwert: ASCII `public-diagnostic-value`.
Binär-Testwert: `[0, 255, 128, 0, 1, 2, 3, 4]`.
Die künstlichen Einträge unter Service `com.bitwarden.desktop.next.diagnostic` wurden wieder gelöscht. Es wurden keine echten gespeicherten Geheimnisse ausgelesen.

Wichtig: Das Testprogramm meldet zuletzt auch für den binären Fall `Ok(())`, weil sein äußerer Result-Wert die erfolgreiche Bereinigung ausdrückt. Entscheidend ist `readback matches = Ok(false)`; der Binär-Roundtrip ist fehlgeschlagen.

Die lokale Quelle von `keyring 3.6.3` zeigt: Auch `set_secret(&[u8])` übergibt beim Erzeugen eines Secret-Service-Items den Content-Type `text/plain`. In dieser KWallet-5-Konstellation kommen bestimmte Binärdaten verändert zurück. Eine Text-/UTF-8-Konvertierung ist eine plausible Erklärung; die genaue Konvertierungsstelle im KWallet-Code wurde nicht untersucht.

Der Client speichert zufällige 32-Byte-AES-Schlüssel mit `set_secret`. Beim Lesen prüft `SecretStore::get` die Länge auf exakt 32 Byte und meldet sonst `Invalid credential-store key`. Der Nutzer beobachtete diese Meldung nach Behebung des QCA-Problems. Sie passt zur nachgewiesenen Datenveränderung; die konkreten Nutzerschlüssel wurden nicht untersucht.

## Empfohlene Client-Korrektur

1. Binäre Schlüssel als versionierten ASCII-Text speichern, beispielsweise Prefix plus Base64. Beim Lesen strikt dekodieren und anschließend die erwartete Schlüssellänge prüfen. Base64 ist nur die Transportkodierung; der OS-Credential-Store bleibt der geschützte Speicher.
2. Eine explizite Kompatibilitätsstrategie für vorhandene rohe 32-Byte-Einträge auf funktionierenden Backends festlegen. Beschädigte Altwerte können nicht allgemein rekonstruiert werden. Keine pauschale Löschung der Wallet.
3. Für beschädigte lokale Credential-Schlüssel gezielt erneute Anmeldung bzw. erneuten Zertifikatsimport anbieten. Welche Wiederherstellung erforderlich ist, hängt vom betroffenen verschlüsselten lokalen Objekt ab.
4. Fehlerursachen erhalten: `OsKeyStore::put` ersetzt gegenwärtig jeden Keyring-Fehler durch „The OS credential store is locked or unavailable“. Verbindungs-/Pluginfehler, gesperrte Collection, fehlender Eintrag und ungültiges Schlüsselformat unterscheiden. Diagnoselogging ohne Schlüssel, Passwörter oder Token.
5. Regressionstests: beliebige 32-Byte-Schlüssel einschließlich Nullbytes und ungültiger UTF-8-Sequenzen, neuer Text-Roundtrip, definierte Altformat-Kompatibilität, beschädigte/ungültige Formate. Mindestens einen echten KWallet-5-Integrationstest neben GNOME Keyring vorsehen; reine Memory-KeyStore-Tests erkennen diesen Fehler nicht.

Am Client-Code wurde in dieser Sitzung noch keine Korrektur vorgenommen.

## Debian-/Ubuntu-Paketanforderungen

Im hier verfügbaren Repository enthält `Packager.toml` derzeit:

```toml
[deb]
depends = ["libayatana-appindicator3-1"]
```

Weitere explizite Secret-Service-, QCA- oder Pinentry-Anforderungen wurden in der untersuchten Packaging-Konfiguration nicht gefunden. Die tatsächlich gebaute DEB-Control-Datei und eventuell zusätzliche Buildskripte auf dem anderen PC müssen noch abgeglichen werden; diese Liste beweist nicht, dass das ausgelieferte DEB nur diese Abhängigkeit besitzt.

Für den Packaging-Folgeauftrag:

- Shared-Library-Laufzeitabhängigkeiten aus dem tatsächlichen Binary distributionsgerecht ermitteln (z. B. `dpkg-shlibdeps`/entsprechende Packaging-Integration), nicht nur manuell aufzählen.
- Eine erreichbare D-Bus-Benutzersitzung und einen nutzbaren Secret-Service-Anbieter als Laufzeitvoraussetzung dokumentieren und prüfen. Ein installiertes Paket alleine genügt nicht.
- Für Ubuntu 24.04/KWallet 5 wurde `libqca-qt5-2-plugins` als erforderlich für den hier verwendeten verschlüsselten DH-Secret-Service-Pfad nachgewiesen. Das ist eine Backend-Abhängigkeit, keine direkte Link-Abhängigkeit des Clients; automatische ELF-Abhängigkeitsanalyse erkennt sie nicht zwingend.
- Bei einer GPG-geschützten Wallet muss ein geeigneter grafischer Pinentry vorhanden sein; hier funktioniert `pinentry-qt`. Andere Wallet-Typen und GNOME Keyring benötigen diese Kombination nicht zwingend.
- GNOME Keyring ist ein alternativer Anbieter; `seahorse` bzw. `kwalletmanager` sind Verwaltungsoberflächen und keine zwingende Voraussetzung für den Clientzugriff.
- Qt5-Paketnamen nicht ungeprüft für neuere Debian-/KDE-/Qt6-Versionen übernehmen. Separate unterstützte Distributions-/Desktop-Kombinationen testen und daraus Depends/Recommends oder dokumentierte Backend-Installationspfade ableiten. Nicht beide Secret-Service-Anbieter pauschal aktivieren.
- Paket-Smoke-Test in einer frischen Benutzersitzung: Dienststart, verschlüsselte Session, Unlock-Prompt und verlustfreier Schreib-/Lese-/Löschzyklus. Optional auch Installation ohne Recommends prüfen, um fehlende indirekte Voraussetzungen sichtbar zu machen.

Die Fedora-Installation wurde nicht untersucht. Ein anderes Backend oder eine andere KWallet-Version kann erklären, warum der Fehler dort nicht auffiel; das ist bislang eine Hypothese.

## Änderungen am Webtop

Im laufenden Container wurden installiert:

- `pinentry-qt` `1.2.1-3ubuntu5`
- `libqca-qt5-2-plugins` `2.3.8-1build3`

Beide Pakete wurden in `/opt/containers/webtop/src/Dockerfile` zur bestehenden `apt install`-Zeile hinzugefügt. Kein Image-Neubau durchgeführt; `git diff --check` war erfolgreich.

APT hatte außerdem widersprüchliche `Signed-By`-Angaben für das Steam-Repository. Die gezielten Installationen verwendeten deshalb ausschließlich `/etc/apt/sources.list` und deaktivierten für diesen Aufruf die zusätzlichen Quellen mittels `-o Dir::Etc::sourceparts=/dev/null`. Die Steam-Konfiguration wurde nicht repariert.

Die Meldung zu `org.a11y.Bus` betrifft die Accessibility-Schnittstelle; „clipboard worker exiting“ meldet das Ende des Clipboard-Workers. Beide Logzeilen enthalten nicht die eigentliche Credential-Store-Fehlerursache.

## Diagnose-Artefakte auf diesem Host

- `/tmp/bw-keyring-probe/`: Rust-Testquelle und gebautes Testprogramm; `dbus` wurde für den Diagnosebuild mit `vendored` aktiviert, da Host-Entwicklungspakete fehlten.
- `/tmp/bw-keyring-probe` im Container: kopiertes Testprogramm.
- `/tmp/bw-native-desktop-diagnosis/`: separat geklontes Repository.

Die /tmp-Artefakte sind temporär. Für die weitere Arbeit sind die Befunde und Testfälle oben maßgeblich.

## Port to egui (2026-11-11)

The original investigation above is retained as historical evidence. Its source
changes are preserved in `recovery/dev-wip-20261111-165811` (`8513336`).
The fixes were ported onto the current `egui-migration` branch, including the
Ubuntu 24.04 packaging workflow. New keys use versioned ASCII Base64 with the
byte API for both writing and reading; this also avoids Windows' UTF-16 password
API conversion. Legacy raw 32-byte keys remain readable without rewriting.
The old Iced title/layout edits are represented by egui's shared 18-pixel title
style and native window controls. Both package entry points now scan the app and
browser helper and include libraries loaded at runtime. The wallet acceptance
tests in `docs/testing.md` still require separate fresh desktop sessions.
