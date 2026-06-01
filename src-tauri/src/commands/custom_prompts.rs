use crate::settings::{self, CustomHotkeyPrompt, PromptMode};
use crate::shortcut;
use log::error;
use tauri::AppHandle;

fn find_conflict(app: &AppHandle, hotkey: &str, exclude_id: Option<&str>) -> Option<String> {
    let settings = settings::get_settings(app);

    // Check system bindings
    for binding in settings.bindings.values() {
        if binding.current_binding == hotkey {
            return Some(format!("System shortcut: {}", binding.name));
        }
    }

    // Check other custom prompts
    for prompt in &settings.custom_hotkey_prompts {
        if Some(prompt.id.as_str()) == exclude_id {
            continue;
        }
        if prompt.hotkey == hotkey {
            return Some(format!("Custom prompt: {}", prompt.name));
        }
    }

    None
}

#[tauri::command]
#[specta::specta]
pub fn get_custom_hotkey_prompts(app: AppHandle) -> Result<Vec<CustomHotkeyPrompt>, String> {
    let settings = settings::get_settings(&app);
    Ok(settings.custom_hotkey_prompts)
}

#[tauri::command]
#[specta::specta]
pub fn create_custom_hotkey_prompt(
    app: AppHandle,
    name: String,
    hotkey: String,
    prompt_text: String,
    mode: String,
) -> Result<CustomHotkeyPrompt, String> {
    let name = name.trim().to_string();
    let hotkey = hotkey.trim().to_string();
    let prompt_text = prompt_text.trim().to_string();

    if name.is_empty() {
        return Err("Prompt name is required".to_string());
    }
    if hotkey.is_empty() {
        return Err("Hotkey is required".to_string());
    }
    if prompt_text.is_empty() {
        return Err("Prompt instructions are required".to_string());
    }

    let mode = match mode.as_str() {
        "selection" => PromptMode::Selection,
        "voice" => PromptMode::Voice,
        _ => return Err("Invalid mode. Must be 'selection' or 'voice'".to_string()),
    };

    // Check for unique name
    let settings = settings::get_settings(&app);
    if settings
        .custom_hotkey_prompts
        .iter()
        .any(|p| p.name == name)
    {
        return Err("A prompt with this name already exists".to_string());
    }

    // Check for hotkey conflicts
    if let Some(conflict) = find_conflict(&app, &hotkey, None) {
        return Err(format!("This hotkey is already used by: {}", conflict));
    }

    let id = format!("prompt_{}", chrono::Utc::now().timestamp_millis());
    let prompt = CustomHotkeyPrompt {
        id: id.clone(),
        name,
        hotkey: hotkey.clone(),
        prompt_text,
        mode,
    };

    let mut settings = settings::get_settings(&app);
    settings.custom_hotkey_prompts.push(prompt.clone());
    settings::write_settings(&app, settings);

    // Register the hotkey
    if let Err(e) = shortcut::register_custom_prompt_hotkey(&app, &prompt) {
        error!("Failed to register custom prompt hotkey: {}", e);
        // Still return the prompt — user can fix hotkey later
    }

    Ok(prompt)
}

#[tauri::command]
#[specta::specta]
pub fn update_custom_hotkey_prompt(
    app: AppHandle,
    id: String,
    name: String,
    hotkey: String,
    prompt_text: String,
    mode: String,
) -> Result<CustomHotkeyPrompt, String> {
    let name = name.trim().to_string();
    let hotkey = hotkey.trim().to_string();
    let prompt_text = prompt_text.trim().to_string();

    if name.is_empty() {
        return Err("Prompt name is required".to_string());
    }
    if hotkey.is_empty() {
        return Err("Hotkey is required".to_string());
    }
    if prompt_text.is_empty() {
        return Err("Prompt instructions are required".to_string());
    }

    let mode = match mode.as_str() {
        "selection" => PromptMode::Selection,
        "voice" => PromptMode::Voice,
        _ => return Err("Invalid mode. Must be 'selection' or 'voice'".to_string()),
    };

    let settings = settings::get_settings(&app);

    // Check for unique name (excluding self)
    if settings
        .custom_hotkey_prompts
        .iter()
        .any(|p| p.id != id && p.name == name)
    {
        return Err("A prompt with this name already exists".to_string());
    }

    // Find existing to get old hotkey for unregister
    let old_prompt = settings
        .custom_hotkey_prompts
        .iter()
        .find(|p| p.id == id)
        .cloned();

    // Check for hotkey conflicts (excluding self)
    if let Some(conflict) = find_conflict(&app, &hotkey, Some(&id)) {
        return Err(format!("This hotkey is already used by: {}", conflict));
    }

    let new_prompt = CustomHotkeyPrompt {
        id: id.clone(),
        name,
        hotkey: hotkey.clone(),
        prompt_text,
        mode,
    };

    let mut settings = settings::get_settings(&app);
    if let Some(pos) = settings
        .custom_hotkey_prompts
        .iter()
        .position(|p| p.id == id)
    {
        // Unregister old hotkey if it changed
        if let Some(old) = &old_prompt {
            if old.hotkey != new_prompt.hotkey {
                let _ = shortcut::unregister_custom_prompt_hotkey(&app, old);
            }
        }

        settings.custom_hotkey_prompts[pos] = new_prompt.clone();
        settings::write_settings(&app, settings);

        // Register new hotkey
        if let Err(e) = shortcut::register_custom_prompt_hotkey(&app, &new_prompt) {
            error!("Failed to register custom prompt hotkey: {}", e);
        }

        Ok(new_prompt)
    } else {
        Err("Prompt not found".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub fn delete_custom_hotkey_prompt(app: AppHandle, id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Find prompt to unregister its hotkey
    if let Some(prompt) = settings
        .custom_hotkey_prompts
        .iter()
        .find(|p| p.id == id)
        .cloned()
    {
        let _ = shortcut::unregister_custom_prompt_hotkey(&app, &prompt);
    }

    let len_before = settings.custom_hotkey_prompts.len();
    settings.custom_hotkey_prompts.retain(|p| p.id != id);

    if settings.custom_hotkey_prompts.len() == len_before {
        return Err("Prompt not found".to_string());
    }

    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn check_custom_prompt_hotkey_conflict(
    app: AppHandle,
    hotkey: String,
    exclude_id: Option<String>,
) -> Result<Option<String>, String> {
    Ok(find_conflict(&app, &hotkey, exclude_id.as_deref()))
}
