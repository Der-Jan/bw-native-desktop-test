# Native vault sorting requirements from iOS

Status: column sorting implemented, full iOS parity pending, 2026-11-07. Sorting must not
change the [mTLS credential destination rules](mtls-requirements.md).

## iOS reference

Use branch `codex/ios-login-sort-options` at
[commit 3e5e9e6](https://github.com/Der-Jan/bitwarden-ios/commit/3e5e9e6afb7b447848153673ca9e56559274db1f).
It adds persistent sorting to detailed vault lists to compare multiple imported
logins for the same service. It is separate from the mTLS branch, and was ahead
of `main` at review time.

The behavior is defined in
[VaultListSortOrder](https://github.com/Der-Jan/bitwarden-ios/blob/3e5e9e6afb7b447848153673ca9e56559274db1f/BitwardenShared/UI/Vault/Vault/VaultList/VaultListSortOrder.swift),
its [tests](https://github.com/Der-Jan/bitwarden-ios/blob/3e5e9e6afb7b447848153673ca9e56559274db1f/BitwardenShared/UI/Vault/Vault/VaultList/VaultListSortOrderTests.swift),
and [AppSettingsStore](https://github.com/Der-Jan/bitwarden-ios/blob/3e5e9e6afb7b447848153673ca9e56559274db1f/BitwardenShared/Core/Platform/Services/Stores/AppSettingsStore.swift).

## Implemented desktop columns

The Name, Last changed and Base URL headers select sorting and toggle ascending /
descending on another click. A newly selected date column starts newest first; the
others start ascending. The active column shows its direction. The application saves
column and direction in `settings.json` and restores them after restart. Unknown enum
values fall back to Name / ascending without discarding other preferences.

Name sorting lowercases Unicode and compares ASCII digit runs numerically, without
integer-size limits. This is deterministic natural ordering, not full locale collation.
Base URL sorts the displayed web hostname plus non-default port, using the same URI
parser as the column. It is deliberately distinct from the iOS raw URL and registrable
base-domain modes below. Missing web URLs sort last in both directions. Ties compare
name ascending and then stable cipher ID, including when the primary order descends.

The list prepares transient name/URL keys when loading account data, retains filters
and search, reselects the same cipher by ID, and resets vertical scroll after reorder.
Lock/logout removes these keys with the account's item cache. Sorting never fetches
websites, DNS records, or full password-bearing cipher details. Magnify is unchanged.

## Required native behavior

| Mode | Ordering |
| --- | --- |
| Name | Natural, case-insensitive localized name ordering; initial default |
| URL | First nonempty login URI in lexical/natural localized order, retaining its full value; missing URI last |
| Base domain | Registrable domain ascending, ignoring scheme/subdomain; missing or invalid domain last |
| Last updated | Server revision date descending, most recently modified first |

For ties, compare name and then stable cipher ID. Base domains use a maintained
public suffix implementation, including multipart suffixes such as `co.uk`;
never take just the last two labels. Match the iOS helper's host fallback for
bare hosts, localhost, IPv4 and IPv6, with explicit test vectors. URL mode uses
the raw first nonempty URI; it does not silently adopt base-domain normalization.

Persist the selected mode as an application-level enum, matching the iOS setting
scope, with unknown values falling back to Name. Store only the enum in ordinary
settings. Sorting keys derived from decrypted names/URLs remain transient account
data and are purged on lock/logout. Do not resolve DNS or contact sites to sort.

Place a sort picker on detailed cipher lists. Sort only cipher rows within each
section; preserve section order, headers, group/navigation rows and verification
code rows. Retain filters/search; recompute after sync without resetting the
chosen mode. Preserve selection by cipher ID, not the previous row index, so
copy/edit cannot act on a different login after reorder. Reconcile virtualization
and scroll state when the visible order changes. Magnify ranking remains a
separate feature unless explicitly brought into scope.

## Implementation and acceptance

Add a pure comparator/key builder in the search/vault domain service, persist the
enum through settings, and apply it to the filtered list in
`views/vault/update/list.rs`. Avoid repeated URL parsing inside every comparator
call; compute transient per-account keys when the snapshot changes. Current
SDK list projections must be checked for URI/revision availability before choosing
the projection or fetching additional data. Never require full password-bearing
decryption just to sort if a safe list projection provides these fields.

Port the iOS fixtures for natural names (`account 2`, `Account 10`), equal-key ID
ties, same base domain with different subdomains, multipart suffixes, bare hosts,
ports, IPs, malformed and empty URIs, first nonempty URI and descending dates.
Add Unicode/IDNA, locale and unknown-setting cases on Rust. Assert section/group
positions, selection after reorder, preference after restart and refresh, and
clearing derived keys on lock. Exact locale collation across operating systems
may differ; deterministic ties and domain extraction must remain consistent.
