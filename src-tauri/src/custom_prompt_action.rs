use crate::actions::ShortcutAction;
use crate::audio_toolkit::{is_microphone_access_denied, is_no_input_device_error};
use crate::clipboard;
use crate::llm_client;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{
    self, AppSettings, CustomHotkeyPrompt, PromptMode, APPLE_INTELLIGENCE_PROVIDER_ID,
    CUSTOM_PROMPT_PREFIX,
};
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, show_processing_overlay, show_recording_overlay, show_transcribing_overlay,
};
use log::{debug, error, warn};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, serde::Serialize)]
struct RecordingErrorEvent {
    error_type: String,
    detail: Option<String>,
}

fn find_prompt<'a>(settings: &'a AppSettings, binding_id: &str) -> Option<&'a CustomHotkeyPrompt> {
    let prompt_id = binding_id.strip_prefix(CUSTOM_PROMPT_PREFIX)?;
    settings
        .custom_hotkey_prompts
        .iter()
        .find(|p| p.id == prompt_id)
}

async fn call_llm_with_prompt(
    settings: &AppSettings,
    prompt: &CustomHotkeyPrompt,
    input_text: &str,
) -> Option<String> {
    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            debug!("Custom prompt: no post-process provider configured");
            return None;
        }
    };

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.trim().is_empty() {
        debug!(
            "Custom prompt: provider '{}' has no model configured",
            provider.id
        );
        return None;
    }

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    let processed_prompt = prompt.prompt_text.replace("${output}", input_text);
    debug!(
        "Custom prompt LLM call: length {} chars",
        processed_prompt.len()
    );

    let (reasoning_effort, reasoning) = match provider.id.as_str() {
        "custom" => (Some("none".to_string()), None),
        "openrouter" => (
            None,
            Some(crate::llm_client::ReasoningConfig {
                effort: Some("none".to_string()),
                exclude: Some(true),
            }),
        ),
        _ => (None, None),
    };

    // For Apple Intelligence, use native API
    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let token_limit = model.trim().parse::<i32>().unwrap_or(0);
            return crate::apple_intelligence::process_text_with_system_prompt(
                &processed_prompt,
                "",
                token_limit,
            )
            .ok();
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            return None;
        }
    }

    match llm_client::send_chat_completion(
        &provider,
        api_key,
        &model,
        processed_prompt,
        reasoning_effort,
        reasoning,
    )
    .await
    {
        Ok(Some(content)) => Some(content),
        Ok(None) => {
            error!("Custom prompt LLM response has no content");
            None
        }
        Err(e) => {
            error!("Custom prompt LLM call failed: {}", e);
            None
        }
    }
}

pub struct CustomPromptAction;

impl ShortcutAction for CustomPromptAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let settings = settings::get_settings(app);
        let prompt = match find_prompt(&settings, binding_id) {
            Some(p) => p.clone(),
            None => {
                warn!("Custom prompt not found for binding: {}", binding_id);
                return;
            }
        };

        debug!(
            "CustomPromptAction::start: {} (mode: {:?})",
            prompt.name, prompt.mode
        );

        match prompt.mode {
            PromptMode::Selection => {
                // Try to read selected text, paste result
                if let Some(enigo_state) = app.try_state::<crate::input::EnigoState>() {
                    let mut enigo = match enigo_state.0.lock() {
                        Ok(e) => e,
                        Err(e) => {
                            error!("Failed to lock Enigo: {}", e);
                            return;
                        }
                    };

                    match clipboard::read_selected_text(&mut enigo, app) {
                        Ok(selected_text) => {
                            drop(enigo);
                            let ah = app.clone();
                            let prompt_clone = prompt.clone();
                            tauri::async_runtime::spawn(async move {
                                show_processing_overlay(&ah);
                                let settings = settings::get_settings(&ah);
                                if let Some(result) =
                                    call_llm_with_prompt(&settings, &prompt_clone, &selected_text)
                                        .await
                                {
                                    if !result.is_empty() {
                                        let ah_clone = ah.clone();
                                        ah.run_on_main_thread(move || {
                                            match utils::paste(result, ah_clone.clone()) {
                                                Ok(()) => {
                                                    debug!("Custom prompt selection result pasted")
                                                }
                                                Err(e) => {
                                                    error!(
                                                        "Failed to paste selection result: {}",
                                                        e
                                                    );
                                                    let _ = ah_clone.emit("paste-error", ());
                                                }
                                            }
                                            utils::hide_recording_overlay(&ah_clone);
                                            change_tray_icon(&ah_clone, TrayIconState::Idle);
                                        })
                                        .unwrap_or_else(
                                            |e| {
                                                error!(
                                                    "Failed to run paste on main thread: {:?}",
                                                    e
                                                );
                                                utils::hide_recording_overlay(&ah);
                                                change_tray_icon(&ah, TrayIconState::Idle);
                                            },
                                        );
                                    }
                                } else {
                                    utils::hide_recording_overlay(&ah);
                                    change_tray_icon(&ah, TrayIconState::Idle);
                                }
                            });
                        }
                        Err(_) => {
                            drop(enigo);
                            error!("No text selected for custom prompt '{}'", prompt.name);
                            let _ = app.emit(
                                "recording-error",
                                RecordingErrorEvent {
                                    error_type: "no_selection".to_string(),
                                    detail: Some(
                                        "No text selected. Select text first, or use Voice mode."
                                            .to_string(),
                                    ),
                                },
                            );
                        }
                    }
                } else {
                    error!("Enigo not initialized for custom prompt");
                }
            }
            PromptMode::Voice => {
                // Voice recording mode — similar to TranscribeAction::start
                let tm = app.state::<Arc<TranscriptionManager>>();
                let rm = app.state::<Arc<AudioRecordingManager>>();

                tm.initiate_model_load();
                let rm_clone = Arc::clone(&rm);
                std::thread::spawn(move || {
                    if let Err(e) = rm_clone.preload_vad() {
                        debug!("VAD pre-load failed: {}", e);
                    }
                });

                let binding_id = binding_id.to_string();
                change_tray_icon(app, TrayIconState::Recording);
                show_recording_overlay(app);

                let settings = settings::get_settings(app);
                let is_always_on = settings.always_on_microphone;

                let mut recording_error: Option<String> = None;
                if is_always_on {
                    let rm_clone = Arc::clone(&rm);
                    let app_clone = app.clone();
                    std::thread::spawn(move || {
                        crate::audio_feedback::play_feedback_sound_blocking(
                            &app_clone,
                            crate::audio_feedback::SoundType::Start,
                        );
                        rm_clone.apply_mute();
                    });
                    if let Err(e) = rm.try_start_recording(&binding_id) {
                        recording_error = Some(e);
                    }
                } else {
                    match rm.try_start_recording(&binding_id) {
                        Ok(()) => {
                            let app_clone = app.clone();
                            let rm_clone = Arc::clone(&rm);
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                crate::audio_feedback::play_feedback_sound_blocking(
                                    &app_clone,
                                    crate::audio_feedback::SoundType::Start,
                                );
                                rm_clone.apply_mute();
                            });
                        }
                        Err(e) => {
                            recording_error = Some(e);
                        }
                    }
                }

                if recording_error.is_none() {
                    shortcut::register_cancel_shortcut(app);
                } else {
                    utils::hide_recording_overlay(app);
                    change_tray_icon(app, TrayIconState::Idle);
                    if let Some(err) = recording_error {
                        let error_type = if is_microphone_access_denied(&err) {
                            "microphone_permission_denied"
                        } else if is_no_input_device_error(&err) {
                            "no_input_device"
                        } else {
                            "unknown"
                        };
                        let _ = app.emit(
                            "recording-error",
                            RecordingErrorEvent {
                                error_type: error_type.to_string(),
                                detail: Some(err),
                            },
                        );
                    }
                }
            }
        }
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        // Only relevant for Voice mode — stop recording, transcribe, call LLM, paste
        let settings = settings::get_settings(app);
        let prompt = match find_prompt(&settings, binding_id) {
            Some(p) => p.clone(),
            None => {
                warn!("Custom prompt not found for binding: {}", binding_id);
                return;
            }
        };

        if prompt.mode != PromptMode::Voice {
            return; // Selection mode doesn't use stop
        }

        shortcut::unregister_cancel_shortcut(app);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());
        let prompt_clone = prompt.clone();

        change_tray_icon(app, TrayIconState::Transcribing);
        show_transcribing_overlay(app);

        rm.remove_mute();
        crate::audio_feedback::play_feedback_sound(app, crate::audio_feedback::SoundType::Stop);

        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            if let Some(samples) = rm.stop_recording(&binding_id) {
                if samples.is_empty() {
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    return;
                }

                let file_name = format!("handy-custom-{}.wav", chrono::Utc::now().timestamp());
                let wav_path = hm.recordings_dir().join(&file_name);
                let samples_for_wav = samples.clone();
                let wav_handle = tauri::async_runtime::spawn_blocking(move || {
                    crate::audio_toolkit::save_wav_file(&wav_path, &samples_for_wav)
                });

                let transcription = tm.transcribe(samples);
                let _ = wav_handle.await;

                match transcription {
                    Ok(text) => {
                        if text.is_empty() {
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                            return;
                        }

                        show_processing_overlay(&ah);
                        let settings = settings::get_settings(&ah);
                        let llm_result =
                            call_llm_with_prompt(&settings, &prompt_clone, &text).await;
                        let final_text = llm_result.clone().unwrap_or_else(|| text.clone());

                        if final_text.is_empty() {
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                            return;
                        }

                        let ah_clone = ah.clone();
                        ah.run_on_main_thread(move || {
                            match utils::paste(final_text.clone(), ah_clone.clone()) {
                                Ok(()) => debug!("Custom prompt voice result pasted"),
                                Err(e) => {
                                    error!("Failed to paste custom prompt result: {}", e);
                                    let _ = ah_clone.emit("paste-error", ());
                                }
                            }
                            utils::hide_recording_overlay(&ah_clone);
                            change_tray_icon(&ah_clone, TrayIconState::Idle);
                        })
                        .unwrap_or_else(|e| {
                            error!("Failed to run paste on main thread: {:?}", e);
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                        });

                        // Save to history
                        if let Err(err) = hm.save_entry(
                            file_name,
                            text,
                            true,
                            llm_result.clone(),
                            Some(prompt_clone.prompt_text.clone()),
                        ) {
                            error!("Failed to save history entry: {}", err);
                        }
                    }
                    Err(err) => {
                        error!("Custom prompt transcription failed: {}", err);
                        utils::hide_recording_overlay(&ah);
                        change_tray_icon(&ah, TrayIconState::Idle);
                    }
                }
            } else {
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
            }
        });
    }
}
