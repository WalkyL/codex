use std::sync::Arc;

use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_extension_api::UserInstructionsProvider;
use codex_login::AuthManager;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use serde::Serialize;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::resolve_installation_id;
use crate::session::session::Session;
use crate::session::turn::build_prompt;
use crate::session::turn::built_tools;
use crate::state_db_bridge::StateDbHandle;
use crate::thread_manager::ThreadManager;
use crate::thread_manager::thread_store_from_config;
use codex_extension_api::empty_extension_registry;

#[derive(Debug, Serialize)]
pub struct DebugRuntimeSnapshot {
    pub thread_id: String,
    pub turn_id: String,
    pub cwd: String,
    pub model: String,
    pub model_provider: String,
    pub collaboration_mode: String,
    pub personality: Option<String>,
    pub approval_policy: String,
    pub permission_profile: String,
    pub service_tier: Option<String>,
    pub show_raw_agent_reasoning: bool,
    pub model_context_window: Option<i64>,
    pub auto_compact_token_limit: Option<i64>,
    pub auto_compact_token_limit_scope: String,
    pub reference_context_present: bool,
    pub prompt_input_count: usize,
    pub input_modalities: Vec<String>,
    pub tools_visible_count: usize,
    pub prompt_preview_text_items: usize,
}

#[derive(Debug, Serialize)]
pub struct DebugLiveContinuationSnapshot {
    pub thread_id: String,
    pub active_turn_present: bool,
    pub pending_input_present: bool,
    pub trigger_turn_mailbox_present: bool,
    pub auto_compact_window_number: u64,
    pub total_input_tokens: Option<i64>,
    pub total_cached_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
}

/// Build the model-visible `input` list for a single debug turn.
#[doc(hidden)]
pub async fn build_prompt_input(
    mut config: Config,
    input: Vec<UserInput>,
    state_db: Option<StateDbHandle>,
    user_instructions_provider: Arc<dyn UserInstructionsProvider>,
) -> CodexResult<Vec<ResponseItem>> {
    config.ephemeral = true;

    let auth_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await;

    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        config.codex_self_exe.clone(),
        config.codex_linux_sandbox_exe.clone(),
    )?;

    let thread_store = thread_store_from_config(&config, state_db.clone());
    let installation_id = resolve_installation_id(&config.codex_home).await?;
    let thread_manager = ThreadManager::new(
        &config,
        Arc::clone(&auth_manager),
        SessionSource::Exec,
        Arc::new(
            EnvironmentManager::from_codex_home(
                config.codex_home.clone(),
                Some(local_runtime_paths),
            )
            .await
            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
        ),
        empty_extension_registry(),
        user_instructions_provider,
        /*analytics_events_client*/ None,
        thread_store,
        crate::local_agent_graph_store_from_state_db(state_db.as_ref()),
        installation_id,
        /*attestation_provider*/ None,
        /*external_time_provider*/ None,
    );
    let thread = thread_manager.start_thread(config).await?;

    let output = build_prompt_input_from_session(&thread.thread.codex.session, input).await;
    let shutdown = thread.thread.shutdown_and_wait().await;
    let _removed = thread_manager.remove_thread(&thread.thread_id).await;

    shutdown?;
    output
}

pub(crate) async fn build_prompt_input_from_session(
    sess: &Arc<Session>,
    input: Vec<UserInput>,
) -> CodexResult<Vec<ResponseItem>> {
    let turn_context = sess.new_default_turn().await;
    // Prompt debugging builds a standalone request without entering run_turn.
    let step_context = sess.capture_step_context(Arc::clone(&turn_context)).await;
    sess.record_context_updates_and_set_reference_context_item(step_context.as_ref())
        .await;

    if !input.is_empty() {
        let response_item = sess.response_item_from_user_input(input);
        sess.record_conversation_items(turn_context.as_ref(), std::slice::from_ref(&response_item))
            .await;
    }

    let prompt_input = sess
        .clone_history()
        .await
        .for_prompt(&turn_context.model_info.input_modalities);
    let router = built_tools(sess, step_context.as_ref(), &CancellationToken::new()).await?;
    let base_instructions = sess.get_base_instructions().await;
    let prompt = build_prompt(
        prompt_input,
        router.as_ref(),
        turn_context.as_ref(),
        base_instructions,
    );

    Ok(prompt.input)
}

pub async fn build_runtime_snapshot(
    mut config: Config,
    input: Vec<UserInput>,
    state_db: Option<StateDbHandle>,
    user_instructions_provider: Arc<dyn UserInstructionsProvider>,
) -> CodexResult<DebugRuntimeSnapshot> {
    config.ephemeral = true;

    let auth_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await;

    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        config.codex_self_exe.clone(),
        config.codex_linux_sandbox_exe.clone(),
    )?;

    let thread_store = thread_store_from_config(&config, state_db.clone());
    let installation_id = resolve_installation_id(&config.codex_home).await?;
    let thread_manager = ThreadManager::new(
        &config,
        Arc::clone(&auth_manager),
        SessionSource::Exec,
        Arc::new(
            EnvironmentManager::from_codex_home(
                config.codex_home.clone(),
                Some(local_runtime_paths),
            )
            .await
            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
        ),
        empty_extension_registry(),
        user_instructions_provider,
        /*analytics_events_client*/ None,
        thread_store,
        crate::local_agent_graph_store_from_state_db(state_db.as_ref()),
        installation_id,
        /*attestation_provider*/ None,
        /*external_time_provider*/ None,
    );
    let thread = thread_manager.start_thread(config).await?;
    let sess = &thread.thread.codex.session;
    let turn_context = sess.new_default_turn().await;
    let step_context = sess.capture_step_context(Arc::clone(&turn_context)).await;
    sess.record_context_updates_and_set_reference_context_item(step_context.as_ref())
        .await;

    if !input.is_empty() {
        let response_item = sess.response_item_from_user_input(input);
        sess.record_conversation_items(turn_context.as_ref(), std::slice::from_ref(&response_item))
            .await;
    }

    let prompt_input = sess
        .clone_history()
        .await
        .for_prompt(&turn_context.model_info.input_modalities);
    let router = built_tools(sess, step_context.as_ref(), &CancellationToken::new()).await?;
    let reference_context_present = sess.reference_context_item().await.is_some();

    let snapshot = DebugRuntimeSnapshot {
        thread_id: sess.thread_id.to_string(),
        turn_id: turn_context.sub_id.clone(),
        cwd: turn_context.cwd.display().to_string(),
        model: turn_context.model_info.slug.clone(),
        model_provider: turn_context.config.model_provider_id.clone(),
        collaboration_mode: format!("{:?}", turn_context.collaboration_mode.mode),
        personality: turn_context.personality.map(|personality| format!("{personality:?}")),
        approval_policy: format!("{:?}", turn_context.approval_policy.value()),
        permission_profile: format!("{:?}", turn_context.permission_profile()),
        service_tier: turn_context.config.service_tier.clone(),
        show_raw_agent_reasoning: sess.public_show_raw_agent_reasoning(),
        model_context_window: turn_context.model_context_window(),
        auto_compact_token_limit: turn_context.config.model_auto_compact_token_limit,
        auto_compact_token_limit_scope: turn_context
            .config
            .model_auto_compact_token_limit_scope
            .to_string(),
        reference_context_present,
        prompt_input_count: prompt_input.len(),
        input_modalities: turn_context
            .model_info
            .input_modalities
            .iter()
            .map(|modality| format!("{modality:?}"))
            .collect(),
        tools_visible_count: router.model_visible_specs().len(),
        prompt_preview_text_items: prompt_input
            .iter()
            .filter(|item| matches!(item, ResponseItem::Message { .. }))
            .count(),
    };

    let shutdown = thread.thread.shutdown_and_wait().await;
    let _removed = thread_manager.remove_thread(&thread.thread_id).await;
    shutdown?;
    Ok(snapshot)
}

pub async fn build_live_continuation_snapshot_for_config(
    mut config: Config,
    state_db: Option<StateDbHandle>,
    user_instructions_provider: Arc<dyn UserInstructionsProvider>,
) -> CodexResult<DebugLiveContinuationSnapshot> {
    config.ephemeral = true;

    let auth_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await;

    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        config.codex_self_exe.clone(),
        config.codex_linux_sandbox_exe.clone(),
    )?;

    let thread_store = thread_store_from_config(&config, state_db.clone());
    let installation_id = resolve_installation_id(&config.codex_home).await?;
    let thread_manager = ThreadManager::new(
        &config,
        Arc::clone(&auth_manager),
        SessionSource::Exec,
        Arc::new(
            EnvironmentManager::from_codex_home(
                config.codex_home.clone(),
                Some(local_runtime_paths),
            )
            .await
            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
        ),
        empty_extension_registry(),
        user_instructions_provider,
        /*analytics_events_client*/ None,
        thread_store,
        crate::local_agent_graph_store_from_state_db(state_db.as_ref()),
        installation_id,
        /*attestation_provider*/ None,
        /*external_time_provider*/ None,
    );
    let thread = thread_manager.start_thread(config).await?;
    let sess = &thread.thread.codex.session;
    let snapshot = build_live_continuation_snapshot(sess).await?;
    let shutdown = thread.thread.shutdown_and_wait().await;
    let _removed = thread_manager.remove_thread(&thread.thread_id).await;
    shutdown?;
    Ok(snapshot)
}

pub async fn build_live_continuation_snapshot(
    sess: &Arc<Session>,
) -> CodexResult<DebugLiveContinuationSnapshot> {
    let active_turn_present = sess.active_turn.lock().await.is_some();
    let pending_input_present = sess.input_queue.has_pending_input(&sess.active_turn).await;
    let trigger_turn_mailbox_present = sess.input_queue.has_trigger_turn_mailbox_items().await;
    let window_number = sess.auto_compact_window_number().await;
    let token_info = sess.token_usage_info().await;

    Ok(DebugLiveContinuationSnapshot {
        thread_id: sess.thread_id.to_string(),
        active_turn_present,
        pending_input_present,
        trigger_turn_mailbox_present,
        auto_compact_window_number: window_number,
        total_input_tokens: token_info.as_ref().map(|info| info.total_token_usage.input_tokens),
        total_cached_input_tokens: token_info
            .as_ref()
            .map(|info| info.total_token_usage.cached_input_tokens),
        total_output_tokens: token_info.as_ref().map(|info| info.total_token_usage.output_tokens),
    })
}
