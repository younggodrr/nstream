use nstream_ingest::{LiveIngestSession, LivePublishSession};
use std::{collections::HashMap, path::PathBuf};
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct LiveSessionRegistry {
    output_root: PathBuf,
    sessions: Mutex<HashMap<(Uuid, Uuid), ManagedLiveSession>>,
}

struct ManagedLiveSession {
    ingest: LiveIngestSession,
    publisher: Option<LivePublishSession>,
}

impl LiveSessionRegistry {
    pub fn from_env() -> Self {
        Self {
            output_root: std::env::var("NSTREAM_LIVE_HLS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./live-hls")),
            sessions: Mutex::new(HashMap::new()),
        }
    }
    pub async fn start(
        &self,
        tenant_id: Uuid,
        source_url: String,
        publish_url: Option<String>,
    ) -> Result<(Uuid, PathBuf, Option<String>), String> {
        let ingest = LiveIngestSession::start(source_url, &self.output_root)
            .await
            .map_err(|e| e.to_string())?;
        let id = ingest.id;
        let playlist = ingest.playlist.clone();
        let publisher = match publish_url {
            Some(target) => Some(
                LivePublishSession::start(&playlist, target)
                    .await
                    .map_err(|e| e.to_string())?,
            ),
            None => None,
        };
        let publish_status = publisher.as_ref().map(|_| "starting".to_string());
        self.sessions
            .lock()
            .await
            .insert((tenant_id, id), ManagedLiveSession { ingest, publisher });
        Ok((id, playlist, publish_status))
    }
    pub async fn status(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<(bool, PathBuf, Option<bool>)>, String> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.get_mut(&(tenant_id, id)) else {
            return Ok(None);
        };
        let publishing = match &mut session.publisher {
            Some(publisher) => Some(publisher.is_running().await.map_err(|e| e.to_string())?),
            None => None,
        };
        Ok(Some((
            session
                .ingest
                .is_running()
                .await
                .map_err(|e| e.to_string())?,
            session.ingest.playlist.clone(),
            publishing,
        )))
    }
    pub async fn stop(&self, tenant_id: Uuid, id: Uuid) -> Result<bool, String> {
        let Some(mut session) = self.sessions.lock().await.remove(&(tenant_id, id)) else {
            return Ok(false);
        };
        if let Some(publisher) = &mut session.publisher {
            let _ = publisher.stop().await;
        }
        session.ingest.stop().await.map_err(|e| e.to_string())?;
        Ok(true)
    }
}
