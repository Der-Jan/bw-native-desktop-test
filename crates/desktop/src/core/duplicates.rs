use super::*;
use services::vault_navigation::{VaultFilter, VaultSection};
impl Core {
    pub(super) fn cancel_duplicate_scan(&mut self) {
        if let Some((_, cancelled)) = self.duplicate_request.take() {
            cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    pub(super) fn ensure_duplicate_scan(&mut self) {
        if self.filter.section != VaultSection::Duplicates {
            self.cancel_duplicate_scan();
            return;
        }
        let Some(scope) = self.scope() else {
            return;
        };
        let filter = self.filter;
        let Some(cache) = self.current_cache() else {
            return;
        };
        if cache.duplicate_filter == Some(filter)
            && (cache.duplicate_report.is_some()
                || cache.duplicate_error.is_some()
                || self.duplicate_request.is_some())
        {
            return;
        }
        let candidates = super::vault::filter(
            cache,
            VaultFilter {
                section: VaultSection::Active,
                ..filter
            },
            "",
        );
        self.cancel_duplicate_scan();
        let id = uuid::Uuid::new_v4();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.duplicate_request = Some((id, cancelled.clone()));
        let default = self.manager.policy_rules_for(&scope.uid).uri_default;
        let cache = self.caches.get_mut(&scope.uid).unwrap();
        cache.duplicate_filter = Some(filter);
        cache.duplicate_report = None;
        cache.duplicate_error = None;
        self.spawn_scoped(
            scope,
            async move {
                tokio::task::spawn_blocking(move || {
                    services::uri_matching::find(candidates, default, cancelled)
                })
                .await
                .map_err(|_| "Website comparison failed".to_owned())?
            },
            move |core, result| {
                if core
                    .duplicate_request
                    .as_ref()
                    .is_none_or(|(request, _)| *request != id)
                {
                    return;
                }
                core.duplicate_request = None;
                let Some(cache) = core
                    .active
                    .and_then(|uid| core.caches.get_mut(&uid))
                    .filter(|c| c.duplicate_filter == Some(filter))
                else {
                    return;
                };
                match result {
                    Ok(report) => cache.duplicate_report = Some(report),
                    Err(error) => cache.duplicate_error = Some(error),
                }
                core.refilter();
            },
        );
    }
}
