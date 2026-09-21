//! Localized strings for core-generated notifications (cron, webhook,
//! sub-agent, triage events) — see `event_to_notification` in `bus.rs`.
//!
//! Coverage today: en-GB (source), it-IT, de-DE, fr-FR, es-ES, nb-NO — the
//! six locales with committed end-to-end test coverage. Every other
//! supported locale (see [`crate::aura::locale::SUPPORTED_LOCALES`]) falls
//! back to en-GB here, same as an untranslated frontend catalog key would.
//! This is a real, tracked gap — not silently hidden: `strings_for` only
//! ever returns one of the six translated sets or the English fallback, so
//! there is no partial/garbled output for an unlisted locale.
//!
//! Adding a language: add a `NotificationStrings` literal and a match arm
//! in `strings_for`. Field docs describe each template's placeholders.

/// One localized string set. Templates use `{name}` placeholders the
/// caller fills via `.replace("{name}", &value)` — deliberately not the
/// frontend's ICU-lite engine, since Rust-side notification bodies are a
/// handful of simple one-placeholder templates with no plural forms.
pub struct NotificationStrings {
    pub cron_completed_title: &'static str,
    pub cron_failed_title: &'static str,
    /// Template: `{job_id}`.
    pub cron_completed_body: &'static str,
    /// Template: `{job_id}`.
    pub cron_failed_body: &'static str,
    pub webhook_error_title: &'static str,
    /// Template: `{skill_id}`, `{elapsed_ms}`, `{err}`.
    pub webhook_error_body_with_err: &'static str,
    /// Template: `{skill_id}`, `{status_code}`, `{elapsed_ms}`.
    pub webhook_error_body_no_err: &'static str,
    pub subagent_completed_title: &'static str,
    /// Template: `{agent_id}`, `{output_chars}`.
    pub subagent_completed_body: &'static str,
    pub subagent_failed_title: &'static str,
    /// Template: `{agent_id}`, `{error}`.
    pub subagent_failed_body: &'static str,
    /// Template: `{provider}`.
    pub triaged_title: &'static str,
    /// Template: `{score}`.
    pub triaged_escalate_body: &'static str,
    /// Template: `{score}`.
    pub triaged_react_body: &'static str,
}

const EN_GB: NotificationStrings = NotificationStrings {
    cron_completed_title: "Cron job completed",
    cron_failed_title: "Cron job failed",
    cron_completed_body: "Job {job_id} finished successfully.",
    cron_failed_body: "Job {job_id} did not complete — check your cron schedule.",
    webhook_error_title: "Webhook error",
    webhook_error_body_with_err: "{skill_id} webhook failed after {elapsed_ms}ms: {err}",
    webhook_error_body_no_err:
        "{skill_id} webhook returned HTTP {status_code} after {elapsed_ms}ms",
    subagent_completed_title: "Sub-agent finished",
    subagent_completed_body: "{agent_id} produced {output_chars} chars of output.",
    subagent_failed_title: "Sub-agent failed",
    subagent_failed_body: "{agent_id} encountered an error: {error}",
    triaged_title: "High-priority {provider} notification",
    triaged_escalate_body: "Action: escalate (score: {score}%). Routed to orchestrator.",
    triaged_react_body: "Action: react (score: {score}%). Routed for follow-up.",
};

const IT_IT: NotificationStrings = NotificationStrings {
    cron_completed_title: "Attività pianificata completata",
    cron_failed_title: "Attività pianificata non riuscita",
    cron_completed_body: "L'attività {job_id} è terminata con successo.",
    cron_failed_body: "L'attività {job_id} non è stata completata — controlla la pianificazione.",
    webhook_error_title: "Errore webhook",
    webhook_error_body_with_err: "Il webhook {skill_id} non è riuscito dopo {elapsed_ms}ms: {err}",
    webhook_error_body_no_err:
        "Il webhook {skill_id} ha restituito HTTP {status_code} dopo {elapsed_ms}ms",
    subagent_completed_title: "Sotto-agente completato",
    subagent_completed_body: "{agent_id} ha prodotto {output_chars} caratteri di output.",
    subagent_failed_title: "Sotto-agente non riuscito",
    subagent_failed_body: "{agent_id} ha riscontrato un errore: {error}",
    triaged_title: "Notifica {provider} ad alta priorità",
    triaged_escalate_body:
        "Azione: escalation (punteggio: {score}%). Instradata all'orchestratore.",
    triaged_react_body: "Azione: reazione (punteggio: {score}%). Instradata per un follow-up.",
};

const DE_DE: NotificationStrings = NotificationStrings {
    cron_completed_title: "Geplanter Job abgeschlossen",
    cron_failed_title: "Geplanter Job fehlgeschlagen",
    cron_completed_body: "Job {job_id} wurde erfolgreich abgeschlossen.",
    cron_failed_body: "Job {job_id} wurde nicht abgeschlossen — bitte den Zeitplan prüfen.",
    webhook_error_title: "Webhook-Fehler",
    webhook_error_body_with_err: "Webhook {skill_id} nach {elapsed_ms}ms fehlgeschlagen: {err}",
    webhook_error_body_no_err:
        "Webhook {skill_id} hat nach {elapsed_ms}ms HTTP {status_code} zurückgegeben",
    subagent_completed_title: "Subagent abgeschlossen",
    subagent_completed_body: "{agent_id} hat {output_chars} Zeichen Ausgabe erzeugt.",
    subagent_failed_title: "Subagent fehlgeschlagen",
    subagent_failed_body: "{agent_id} ist auf einen Fehler gestoßen: {error}",
    triaged_title: "Wichtige {provider}-Benachrichtigung",
    triaged_escalate_body:
        "Aktion: Eskalation (Wert: {score}%). An den Orchestrator weitergeleitet.",
    triaged_react_body: "Aktion: Reaktion (Wert: {score}%). Zur Nachverfolgung weitergeleitet.",
};

const FR_FR: NotificationStrings = NotificationStrings {
    cron_completed_title: "Tâche planifiée terminée",
    cron_failed_title: "Échec de la tâche planifiée",
    cron_completed_body: "La tâche {job_id} s'est terminée avec succès.",
    cron_failed_body: "La tâche {job_id} ne s'est pas terminée — vérifiez votre planification.",
    webhook_error_title: "Erreur de webhook",
    webhook_error_body_with_err: "Le webhook {skill_id} a échoué après {elapsed_ms}ms : {err}",
    webhook_error_body_no_err:
        "Le webhook {skill_id} a renvoyé HTTP {status_code} après {elapsed_ms}ms",
    subagent_completed_title: "Sous-agent terminé",
    subagent_completed_body: "{agent_id} a produit {output_chars} caractères de sortie.",
    subagent_failed_title: "Échec du sous-agent",
    subagent_failed_body: "{agent_id} a rencontré une erreur : {error}",
    triaged_title: "Notification {provider} prioritaire",
    triaged_escalate_body: "Action : escalade (score : {score} %). Transmise à l'orchestrateur.",
    triaged_react_body: "Action : réaction (score : {score} %). Transmise pour suivi.",
};

const ES_ES: NotificationStrings = NotificationStrings {
    cron_completed_title: "Tarea programada completada",
    cron_failed_title: "Tarea programada fallida",
    cron_completed_body: "La tarea {job_id} finalizó correctamente.",
    cron_failed_body: "La tarea {job_id} no se completó — revisa tu programación.",
    webhook_error_title: "Error de webhook",
    webhook_error_body_with_err: "El webhook {skill_id} falló tras {elapsed_ms}ms: {err}",
    webhook_error_body_no_err:
        "El webhook {skill_id} devolvió HTTP {status_code} tras {elapsed_ms}ms",
    subagent_completed_title: "Subagente finalizado",
    subagent_completed_body: "{agent_id} generó {output_chars} caracteres de salida.",
    subagent_failed_title: "Subagente fallido",
    subagent_failed_body: "{agent_id} encontró un error: {error}",
    triaged_title: "Notificación de {provider} de alta prioridad",
    triaged_escalate_body: "Acción: escalado (puntuación: {score}%). Enviada al orquestador.",
    triaged_react_body: "Acción: reacción (puntuación: {score}%). Enviada para seguimiento.",
};

const NB_NO: NotificationStrings = NotificationStrings {
    cron_completed_title: "Planlagt jobb fullført",
    cron_failed_title: "Planlagt jobb mislyktes",
    cron_completed_body: "Jobb {job_id} ble fullført.",
    cron_failed_body: "Jobb {job_id} ble ikke fullført — sjekk cron-planen din.",
    webhook_error_title: "Webhook-feil",
    webhook_error_body_with_err: "Webhook {skill_id} mislyktes etter {elapsed_ms}ms: {err}",
    webhook_error_body_no_err:
        "Webhook {skill_id} returnerte HTTP {status_code} etter {elapsed_ms}ms",
    subagent_completed_title: "Underagent fullført",
    subagent_completed_body: "{agent_id} produserte {output_chars} tegn med utdata.",
    subagent_failed_title: "Underagent mislyktes",
    subagent_failed_body: "{agent_id} støtte på en feil: {error}",
    triaged_title: "Viktig {provider}-varsel",
    triaged_escalate_body: "Handling: eskaler (score: {score}%). Sendt til orkestratoren.",
    triaged_react_body: "Handling: reager (score: {score}%). Sendt til oppfølging.",
};

/// Resolve the localized string set for `locale`. Unknown or untranslated
/// locales fall back to en-GB (see module docs for coverage).
pub fn strings_for(locale: &str) -> &'static NotificationStrings {
    match locale {
        "it-IT" => &IT_IT,
        "de-DE" => &DE_DE,
        "fr-FR" => &FR_FR,
        "es-ES" => &ES_ES,
        "nb-NO" => &NB_NO,
        _ => &EN_GB,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_english_for_unlisted_locale() {
        let s = strings_for("pl-PL");
        assert_eq!(s.cron_completed_title, EN_GB.cron_completed_title);
    }

    #[test]
    fn resolves_each_translated_locale() {
        assert_eq!(
            strings_for("it-IT").cron_completed_title,
            "Attività pianificata completata"
        );
        assert_eq!(
            strings_for("de-DE").cron_completed_title,
            "Geplanter Job abgeschlossen"
        );
        assert_eq!(
            strings_for("fr-FR").cron_completed_title,
            "Tâche planifiée terminée"
        );
        assert_eq!(
            strings_for("es-ES").cron_completed_title,
            "Tarea programada completada"
        );
        assert_eq!(
            strings_for("nb-NO").cron_completed_title,
            "Planlagt jobb fullført"
        );
    }

    #[test]
    fn every_locale_has_all_templates_non_empty() {
        for s in [&EN_GB, &IT_IT, &DE_DE, &FR_FR, &ES_ES, &NB_NO] {
            assert!(!s.cron_completed_title.is_empty());
            assert!(!s.cron_failed_title.is_empty());
            assert!(!s.cron_completed_body.is_empty());
            assert!(!s.cron_failed_body.is_empty());
            assert!(!s.webhook_error_title.is_empty());
            assert!(!s.webhook_error_body_with_err.is_empty());
            assert!(!s.webhook_error_body_no_err.is_empty());
            assert!(!s.subagent_completed_title.is_empty());
            assert!(!s.subagent_completed_body.is_empty());
            assert!(!s.subagent_failed_title.is_empty());
            assert!(!s.subagent_failed_body.is_empty());
            assert!(!s.triaged_title.is_empty());
            assert!(!s.triaged_escalate_body.is_empty());
            assert!(!s.triaged_react_body.is_empty());
        }
    }
}
