// meetings/summarize.rs — AI-powered meeting summarization with template support.

/// Template types for meeting summarization.
/// Each drives a different AI prompt structure.
pub enum Template {
    General,
    Standup,
    OneOnOne,
    Retrospective,
    Planning,
}

impl Template {
    pub fn from_str(s: &str) -> Self {
        match s {
            "standup"       => Self::Standup,
            "one_on_one"    => Self::OneOnOne,
            "retrospective" => Self::Retrospective,
            "planning"      => Self::Planning,
            _               => Self::General,
        }
    }

    fn prompt(&self, transcript: &str) -> String {
        match self {
            Self::General => format!(
                "Analizza questa trascrizione di riunione e produci un riassunto strutturato con le sezioni:\n\
                \n## Riassunto\n(2-3 frasi)\n\
                \n## Partecipanti\n(nomi o ruoli menzionati)\n\
                \n## Punti chiave\n(elenco puntato)\n\
                \n## Decisioni\n(elenco delle decisioni prese)\n\
                \n## Azioni\n(cosa fare, chi, entro quando)\n\
                \nTrascrizione:\n{transcript}"
            ),
            Self::Standup => format!(
                "Analizza questo standup meeting e compila le tre sezioni classiche:\n\
                \n## Ieri\n(cosa è stato completato)\n\
                \n## Oggi\n(cosa si farà oggi)\n\
                \n## Blocchi\n(impedimenti o problemi aperti)\n\
                \nAggiungi anche:\n## Azioni\n(task o follow-up emersi)\n\
                \nTrascrizione:\n{transcript}"
            ),
            Self::OneOnOne => format!(
                "Analizza questo one-on-one e organizza il contenuto in:\n\
                \n## Stato attuale\n(aggiornamenti su progetti o obiettivi)\n\
                \n## Feedback\n(feedback reciproco emerso)\n\
                \n## Obiettivi\n(obiettivi discussi o aggiornati)\n\
                \n## Azioni\n(azioni concrete con responsabile)\n\
                \n## Note private\n(eventuali punti sensibili o da ricordare)\n\
                \nTrascrizione:\n{transcript}"
            ),
            Self::Retrospective => format!(
                "Analizza questa retrospettiva e organizza il contenuto in:\n\
                \n## Cosa ha funzionato\n(punti positivi, da mantenere)\n\
                \n## Cosa non ha funzionato\n(problemi emersi, da migliorare)\n\
                \n## Miglioramenti\n(azioni concrete per il prossimo sprint)\n\
                \n## Riconoscimenti\n(persone o lavori apprezzati, se menzionati)\n\
                \nTrascrizione:\n{transcript}"
            ),
            Self::Planning => format!(
                "Analizza questa sessione di planning e organizza il contenuto in:\n\
                \n## Obiettivi dello sprint\n(goal condiviso)\n\
                \n## Backlog selezionato\n(storie o task presi in carico, con stime se disponibili)\n\
                \n## Assegnazioni\n(chi fa cosa)\n\
                \n## Rischi\n(dipendenze o rischi identificati)\n\
                \n## Azioni pre-sprint\n(task da completare prima di iniziare)\n\
                \nTrascrizione:\n{transcript}"
            ),
        }
    }
}

/// Map a BCP-47 language tag to a human-readable name for the AI system prompt.
fn lang_name(lang: &str) -> &'static str {
    match lang.get(..2).unwrap_or("it") {
        "en" => "English",
        "de" => "German (Deutsch)",
        "fr" => "French (Français)",
        "es" => "Spanish (Español)",
        "pt" => "Portuguese (Português)",
        _ => "Italian (Italiano)",
    }
}

/// Summarize a meeting transcript using the given template type and output language.
/// `lang` is a BCP-47 tag (e.g. "it-IT", "en-US"). Defaults to Italian when empty.
pub async fn summarize_with_template(transcript: &str, template: &str, lang: &str) -> Result<String, String> {
    let cfg = crate::ai::load_config();
    if cfg.provider == "none" {
        return Err("no AI provider configured".into());
    }
    let output_lang = lang_name(lang);
    let system = format!(
        "You are a specialized meeting analysis assistant. \
         Always produce output in structured Markdown format. \
         Do not use emoji, icons, or decorative symbols — plain text only. \
         Output language: {output_lang}. Use section titles in that language."
    );
    let prompt = Template::from_str(template).prompt(transcript);
    let messages = vec![serde_json::json!({"role": "user", "content": prompt})];
    crate::ai::chat_completion(messages, &system, &cfg).await
}

/// Convenience wrapper using the general template in Italian.
pub async fn summarize_transcript(transcript: &str) -> Result<String, String> {
    summarize_with_template(transcript, "general", "it-IT").await
}

/// Extract a concise meeting title from the first lines of the transcript.
pub fn extract_title_from_transcript(transcript: &str) -> String {
    let seed: String = transcript.chars().take(120).collect();
    let first = seed.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("Riunione");

    if first.len() > 60 {
        format!("{}…", first.chars().take(57).collect::<String>())
    } else {
        first.to_string()
    }
}
