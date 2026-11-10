use super::*;
use services::accounts::proxy::Broker;
impl Core {
    pub fn proxy_broker(&self) -> Option<Broker> {
        self.profiles
            .as_ref()
            .map(|s| s.proxy.clone())
            .or_else(|| self.manager.profiles().map(|s| s.proxy.clone()))
    }
    pub(super) fn poll_proxy(&mut self) {
        let Some(broker) = self.proxy_broker() else {
            return;
        };
        let events = self.events.clone();
        broker.set_wake(Arc::new(move || {
            (events.wake)();
            let _ = events.sender.try_send(Box::new(|core| core.poll_proxy()));
            (events.wake)();
        }));
        if self
            .proxy_auth
            .as_ref()
            .is_some_and(|form| !broker.current(form.challenge.id))
        {
            self.proxy_auth = None;
            self.clear_epoch = self
                .clear_epoch
                .checked_add(1)
                .expect("proxy cleanup epoch");
        }
        if self.proxy_auth.is_none()
            && let Some(challenge) = broker.pending()
        {
            self.proxy_auth = Some(model::ProxyAuth {
                challenge,
                username: Zeroizing::new(String::new()),
                password: Zeroizing::new(String::new()),
                error: None,
            });
        }
    }
    pub fn submit_proxy(&mut self) {
        let Some(broker) = self.proxy_broker() else {
            return;
        };
        let Some(form) = self.proxy_auth.as_ref() else {
            return;
        };
        let result = broker.submit(
            form.challenge.id,
            Zeroizing::new(form.username.to_string()),
            Zeroizing::new(form.password.to_string()),
        );
        match result {
            Ok(()) => {
                self.proxy_auth = None;
                self.clear_epoch = self
                    .clear_epoch
                    .checked_add(1)
                    .expect("proxy cleanup epoch");
                self.notify("Proxy credentials saved for this app session. Retry sign-in or synchronize; writes were not resent.",false);
                self.check_incoming_now();
            }
            Err(error) => {
                if let Some(form) = self.proxy_auth.as_mut() {
                    form.error = Some(error);
                }
            }
        }
    }
    pub fn cancel_proxy(&mut self) {
        if let Some(broker) = self.proxy_broker() {
            broker.cancel_pending();
        }
        self.proxy_auth = None;
        self.clear_epoch = self
            .clear_epoch
            .checked_add(1)
            .expect("proxy cleanup epoch");
    }
}
