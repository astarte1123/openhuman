// composio.rs — Composio integration for Youma.
//
// One COMPOSIO_API_KEY gives access to 118+ third-party service connections
// (Gmail, GitHub, Notion, Slack, Stripe, Linear, Jira, …).  The user
// authenticates each service via OAuth or API-key flows managed entirely by
// Composio — Youma only stores the connection record and polls for data.
//
// API reference: https://backend.composio.dev/api/v1

use reqwest::Client;
use serde_json::{json, Value};

use crate::db;

const COMPOSIO_BASE: &str = "https://backend.composio.dev/api/v1";

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct ComposioClient {
    pub api_key: String,
    client: Client,
}

impl ComposioClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("composio client build"),
        }
    }

    // -----------------------------------------------------------------------
    // Apps — catalogue of available integrations
    // -----------------------------------------------------------------------

    pub async fn list_apps(&self) -> Result<Vec<Value>, String> {
        eprintln!("[composio] list_apps");
        let resp = self.client
            .get(format!("{COMPOSIO_BASE}/apps"))
            .header("x-api-key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("[composio] list_apps request: {e}"))?;

        let status = resp.status();
        let body: Value = resp.json().await
            .map_err(|e| format!("[composio] list_apps parse: {e}"))?;

        if !status.is_success() {
            return Err(format!("[composio] list_apps HTTP {status}: {body}"));
        }

        // Response: { "items": [...] }  or  plain array
        let apps = body.get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_else(|| body.as_array().cloned().unwrap_or_default());

        let normalised: Vec<Value> = apps.iter().map(|a| {
            let key = a.get("key").or_else(|| a.get("name"))
                .and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let display = a.get("name").and_then(|v| v.as_str())
                .unwrap_or(&key).to_string();
            json!({
                "name": key,
                "display_name": display,
                "description": a.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                "auth_mode": a.get("auth_scheme").or_else(|| a.get("authScheme"))
                    .and_then(|v| v.as_str()).unwrap_or("OAUTH2"),
                "logo_url": a.get("logo").or_else(|| a.get("logoUrl")).or_else(|| a.get("logo_url"))
                    .and_then(|v| v.as_str()).unwrap_or(""),
            })
        }).collect();

        eprintln!("[composio] list_apps → {} apps", normalised.len());
        Ok(normalised)
    }

    // -----------------------------------------------------------------------
    // Connections — user's connected accounts
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub async fn list_connections(&self) -> Result<Vec<Value>, String> {
        eprintln!("[composio] list_connections");
        let resp = self.client
            .get(format!("{COMPOSIO_BASE}/connectedAccounts"))
            .header("x-api-key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("[composio] list_connections request: {e}"))?;

        let status = resp.status();
        let body: Value = resp.json().await
            .map_err(|e| format!("[composio] list_connections parse: {e}"))?;

        if !status.is_success() {
            return Err(format!("[composio] list_connections HTTP {status}: {body}"));
        }

        let conns = body.get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        eprintln!("[composio] list_connections → {} connections", conns.len());
        Ok(conns)
    }

    // -----------------------------------------------------------------------
    // Initiate OAuth / API-key connection — returns redirect URL
    // -----------------------------------------------------------------------

    pub async fn initiate_connection(
        &self,
        app_name: &str,
        entity_id: &str,
        redirect_url: Option<&str>,
    ) -> Result<Value, String> {
        eprintln!("[composio] initiate_connection app={app_name} entity={entity_id}");

        let mut body = json!({
            "appName": app_name,
            "entityId": entity_id,
        });
        if let Some(url) = redirect_url {
            body["redirectUri"] = json!(url);
        }

        let resp = self.client
            .post(format!("{COMPOSIO_BASE}/connectedAccounts"))
            .header("x-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("[composio] initiate_connection request: {e}"))?;

        let status = resp.status();
        let resp_body: Value = resp.json().await
            .map_err(|e| format!("[composio] initiate_connection parse: {e}"))?;

        if !status.is_success() {
            return Err(format!("[composio] initiate_connection HTTP {status}: {resp_body}"));
        }

        eprintln!("[composio] initiate_connection → connectedAccountId={:?}",
            resp_body.get("connectedAccountId"));
        Ok(resp_body)
    }

    // -----------------------------------------------------------------------
    // Poll connection status
    // -----------------------------------------------------------------------

    pub async fn poll_connection_status(&self, connection_id: &str) -> Result<Value, String> {
        eprintln!("[composio] poll_connection_status id={connection_id}");
        let resp = self.client
            .get(format!("{COMPOSIO_BASE}/connectedAccounts/{connection_id}"))
            .header("x-api-key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("[composio] poll_connection_status request: {e}"))?;

        let status = resp.status();
        let body: Value = resp.json().await
            .map_err(|e| format!("[composio] poll_connection_status parse: {e}"))?;

        if !status.is_success() {
            return Err(format!("[composio] poll_connection_status HTTP {status}: {body}"));
        }
        Ok(body)
    }

    // -----------------------------------------------------------------------
    // Delete (disconnect) a connection
    // -----------------------------------------------------------------------

    pub async fn delete_connection(&self, connection_id: &str) -> Result<(), String> {
        eprintln!("[composio] delete_connection id={connection_id}");
        let resp = self.client
            .delete(format!("{COMPOSIO_BASE}/connectedAccounts/{connection_id}"))
            .header("x-api-key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("[composio] delete_connection request: {e}"))?;

        if !resp.status().is_success() {
            let s = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("[composio] delete_connection HTTP {s}: {body}"));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Execute an action on a connected account
    // -----------------------------------------------------------------------

    pub async fn execute_action(
        &self,
        action_id: &str,
        entity_id: &str,
        input: Value,
    ) -> Result<Value, String> {
        eprintln!("[composio] execute_action action={action_id} entity={entity_id}");

        let body = json!({
            "entityId": entity_id,
            "input": input,
        });

        let resp = self.client
            .post(format!("{COMPOSIO_BASE}/actions/{action_id}/execute"))
            .header("x-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("[composio] execute_action request: {e}"))?;

        let status = resp.status();
        let resp_body: Value = resp.json().await
            .map_err(|e| format!("[composio] execute_action parse: {e}"))?;

        if !status.is_success() {
            return Err(format!("[composio] execute_action HTTP {status}: {resp_body}"));
        }
        Ok(resp_body)
    }
}

// ---------------------------------------------------------------------------
// Client factory
// ---------------------------------------------------------------------------

pub fn load_client() -> Option<ComposioClient> {
    let key = std::env::var("COMPOSIO_API_KEY").unwrap_or_default();
    if key.is_empty() { return None; }
    Some(ComposioClient::new(&key))
}

// ---------------------------------------------------------------------------
// Background auto-fetch
// ---------------------------------------------------------------------------

/// Map of well-known Composio app slugs → default list/fetch action IDs.
fn default_fetch_action(app_name: &str) -> Option<&'static str> {
    match app_name.to_lowercase().as_str() {
        "gmail"                               => Some("GMAIL_FETCH_EMAILS"),
        "googlecalendar" | "google_calendar"  => Some("GOOGLECALENDAR_LIST_EVENTS"),
        "googledrive"    | "google_drive"     => Some("GOOGLEDRIVE_LIST_FILES"),
        "notion"                              => Some("NOTION_SEARCH_NOTION_PAGE"),
        "github"                              => Some("GITHUB_LIST_EVENTS_FOR_AUTHENTICATED_USER"),
        "slack"                               => Some("SLACK_LIST_ALL_SLACK_CHANNEL"),
        "linear"                              => Some("LINEAR_LIST_LINEAR_ISSUES"),
        "jira"                                => Some("JIRA_LIST_JIRA_ISSUES"),
        "stripe"                              => Some("STRIPE_LIST_CHARGES"),
        "hubspot"                             => Some("HUBSPOT_LIST_CONTACTS"),
        "asana"                               => Some("ASANA_LIST_TASKS"),
        "trello"                              => Some("TRELLO_LIST_BOARDS_FOR_USER"),
        "discord"                             => Some("DISCORD_LIST_CHANNELS"),
        "dropbox"                             => Some("DROPBOX_LIST_FOLDER"),
        "airtable"                            => Some("AIRTABLE_LIST_RECORDS"),
        _                                     => None,
    }
}

/// Run auto-fetch for every ACTIVE connection, truncate the result, and save
/// it as a memory note so the AI has recent context.
/// Called every 20 minutes by the background tokio task in `main.rs`.
pub async fn auto_fetch_all() {
    eprintln!("[composio] auto_fetch_all started");

    let client = match load_client() {
        Some(c) => c,
        None => {
            eprintln!("[composio] auto_fetch_all: no COMPOSIO_API_KEY — skipping");
            return;
        }
    };

    let active = match db::composio_active_connections() {
        Ok(v)  => v,
        Err(e) => { eprintln!("[composio] auto_fetch_all db error: {e}"); return; }
    };

    if active.is_empty() {
        eprintln!("[composio] auto_fetch_all: no active connections");
        return;
    }

    eprintln!("[composio] auto_fetch_all: {} active connections", active.len());

    for conn_record in &active {
        let app_name  = match conn_record.get("app_name").and_then(|v| v.as_str()) {
            Some(s) => s, None => continue,
        };
        let entity_id = conn_record.get("entity_id").and_then(|v| v.as_str()).unwrap_or("default");
        let conn_id   = conn_record.get("id").and_then(|v| v.as_str()).unwrap_or("");

        let action_id = match default_fetch_action(app_name) {
            Some(a) => a,
            None    => {
                eprintln!("[composio] auto_fetch_all: no default action for {app_name}");
                continue;
            }
        };

        match client.execute_action(action_id, entity_id, json!({})).await {
            Ok(result) => {
                eprintln!("[composio] auto_fetch_all: {app_name} fetch ok");

                // Truncate to ~800 chars and save as memory.
                let raw = serde_json::to_string(&result).unwrap_or_default();
                let summary: String = raw.chars().take(800).collect();
                if !summary.is_empty() {
                    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
                    let content = format!("[{app_name} data {now}] {summary}");
                    let _ = db::memories_create(&content, &format!("auto,composio,{app_name}"));
                    eprintln!("[composio] auto_fetch_all: saved memory for {app_name}");
                }

                if !conn_id.is_empty() {
                    let _ = db::composio_connection_set_fetched(conn_id);
                }
            }
            Err(e) => eprintln!("[composio] auto_fetch_all: {app_name} error: {e}"),
        }
    }
}
