use super::{download::*, system::*};
use crate::jsonrpc::utils::rpc_error;
use aura_core::Engine;
use serde_json::Value;
use std::sync::Arc;

pub struct RpcRouter {
    engine: Arc<Engine>,
}

impl RpcRouter {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine }
    }

    pub async fn route(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> (Result<Value, Value>, Option<bool>) {
        if method == "aura.addUri" {
            return match handle_add_uri(&self.engine, params).await {
                Ok((val, exists)) => (Ok(val), exists),
                Err(err) => (Err(err), None),
            };
        }

        let res = match method {
            "aura.tellActive" => handle_tell_active(&self.engine).await,
            "aura.pause" => handle_pause(&self.engine, params).await,
            "aura.unpause" => handle_unpause(&self.engine, params).await,
            "aura.forceRecheck" => handle_force_recheck(&self.engine, params).await,
            "aura.remove" => handle_remove(&self.engine, params).await,
            "aura.changeOption" => handle_change_option(&self.engine, params).await,
            "aura.refreshUri" => handle_refresh(&self.engine, params).await,
            "aura.getConfig" => handle_get_config(&self.engine).await,
            "aura.getVersion" => handle_get_version().await,
            "aura.getSessionInfo" => handle_get_session_info().await,
            "aura.tellStopped" => handle_tell_stopped(&self.engine, params).await,
            "aura.tellWaiting" => handle_tell_waiting(&self.engine, params).await,
            "aura.getStatus" => handle_get_status(&self.engine, params).await,
            "aura.purgeDownloadResult" => handle_purge_download_result(&self.engine).await,
            "aura.removeDownloadResult" => {
                handle_remove_download_result(&self.engine, params).await
            }
            "aura.saveSession" => handle_save_session().await,
            "aura.shutdown" => handle_shutdown(&self.engine).await,
            "aura.forceShutdown" => handle_shutdown(&self.engine).await,
            "aura.changeGlobalOption" => handle_change_global_option(&self.engine, params).await,
            "aura.getGlobalStat" => handle_get_global_stat(&self.engine).await,
            "aura.getFiles" => handle_get_files(&self.engine, params).await,
            "aura.setFileSelection" => handle_set_file_selection(&self.engine, params).await,
            "aura.addFromFolder" => handle_add_from_folder(&self.engine, params).await,
            "aura.addFromFile" => handle_add_from_file(&self.engine, params).await,
            _ => Err(rpc_error(-32601, "Method not found")),
        };
        (res, None)
    }
}
