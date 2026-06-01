import React, { useEffect, useState } from "react";
import { useTranslation, Trans } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Dropdown, SettingContainer, SettingsGroup, Textarea } from "@/components/ui";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Alert } from "@/components/ui/Alert";
import { HotkeyPromptInput } from "./HotkeyPromptInput";
import { useSettings } from "@/hooks/useSettings";

type PromptMode = "selection" | "voice";

interface CustomHotkeyPrompt {
  id: string;
  name: string;
  hotkey: string;
  prompt_text: string;
  mode: PromptMode;
}

async function getCustomHotkeyPrompts(): Promise<CustomHotkeyPrompt[]> {
  const result: any = await invoke("get_custom_hotkey_prompts");
  return result as CustomHotkeyPrompt[];
}

async function createCustomHotkeyPrompt(
  name: string, hotkey: string, prompt_text: string, mode: string,
): Promise<CustomHotkeyPrompt> {
  const result: any = await invoke("create_custom_hotkey_prompt", { name, hotkey, promptText: prompt_text, mode });
  return result as CustomHotkeyPrompt;
}

async function updateCustomHotkeyPrompt(
  id: string, name: string, hotkey: string, prompt_text: string, mode: string,
): Promise<CustomHotkeyPrompt> {
  const result: any = await invoke("update_custom_hotkey_prompt", { id, name, hotkey, promptText: prompt_text, mode });
  return result as CustomHotkeyPrompt;
}

async function deleteCustomHotkeyPrompt(id: string): Promise<void> {
  await invoke("delete_custom_hotkey_prompt", { id });
}

export const CustomHotkeyPrompts: React.FC = () => {
  const { t } = useTranslation();
  const { refreshSettings } = useSettings();
  const [prompts, setPrompts] = useState<CustomHotkeyPrompt[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftHotkey, setDraftHotkey] = useState("");
  const [draftText, setDraftText] = useState("");
  const [draftMode, setDraftMode] = useState<PromptMode>("selection");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    loadPrompts();
  }, []);

  const loadPrompts = async () => {
    try {
      const data = await getCustomHotkeyPrompts();
      setPrompts(data);
    } catch (e) {
      console.error("Failed to load custom prompts:", e);
    }
  };

  const selectedPrompt = prompts.find((p) => p.id === selectedId) || null;

  useEffect(() => {
    if (isCreating) return;
    if (selectedPrompt) {
      setDraftName(selectedPrompt.name);
      setDraftHotkey(selectedPrompt.hotkey);
      setDraftText(selectedPrompt.prompt_text);
      setDraftMode(selectedPrompt.mode);
    } else {
      setDraftName("");
      setDraftHotkey("");
      setDraftText("");
      setDraftMode("selection");
    }
  }, [selectedId, selectedPrompt?.name, selectedPrompt?.hotkey, selectedPrompt?.prompt_text, selectedPrompt?.mode, isCreating]);

  const validate = (): string | null => {
    if (!draftName.trim()) return t("settings.customPrompts.validation.nameRequired");
    if (!draftHotkey.trim()) return t("settings.customPrompts.validation.hotkeyRequired");
    if (!draftText.trim()) return t("settings.customPrompts.validation.promptRequired");

    const isDuplicateName = prompts.some(
      (p) => p.name === draftName.trim() && p.id !== selectedId,
    );
    if (isDuplicateName) return t("settings.customPrompts.validation.nameUnique");

    return null;
  };

  const handleCreate = async () => {
    const err = validate();
    if (err) { setError(err); return; }
    setSaving(true);
    setError(null);

    try {
      const prompt = await createCustomHotkeyPrompt(
        draftName.trim(), draftHotkey.trim(), draftText.trim(), draftMode,
      );
      await loadPrompts();
      setSelectedId(prompt.id);
      setIsCreating(false);
      await refreshSettings();
    } catch (e: any) {
      setError(typeof e === "string" ? e : e.message || "Failed to create prompt");
    }
    setSaving(false);
  };

  const handleUpdate = async () => {
    if (!selectedId) return;
    const err = validate();
    if (err) { setError(err); return; }
    setSaving(true);
    setError(null);

    try {
      await updateCustomHotkeyPrompt(
        selectedId, draftName.trim(), draftHotkey.trim(), draftText.trim(), draftMode,
      );
      await loadPrompts();
      await refreshSettings();
    } catch (e: any) {
      setError(typeof e === "string" ? e : e.message || "Failed to update prompt");
    }
    setSaving(false);
  };

  const handleDelete = async () => {
    if (!selectedId) return;
    setSaving(true);
    setError(null);

    try {
      await deleteCustomHotkeyPrompt(selectedId);
      await loadPrompts();
      setSelectedId(null);
      setIsCreating(false);
      await refreshSettings();
    } catch (e: any) {
      setError(typeof e === "string" ? e : e.message || "Failed to delete prompt");
    }
    setSaving(false);
  };

  const handleStartCreate = () => {
    setIsCreating(true);
    setSelectedId(null);
    setDraftName("");
    setDraftHotkey("");
    setDraftText("");
    setDraftMode("selection");
    setError(null);
  };

  const handleCancelCreate = () => {
    setIsCreating(false);
    if (selectedPrompt) {
      setDraftName(selectedPrompt.name);
      setDraftHotkey(selectedPrompt.hotkey);
      setDraftText(selectedPrompt.prompt_text);
      setDraftMode(selectedPrompt.mode);
    }
    setError(null);
  };

  const handleDeleteConfirm = () => {
    if (window.confirm(t("settings.customPrompts.deleteConfirm"))) {
      handleDelete();
    }
  };

  const hasPrompts = prompts.length > 0;
  const isDirty = selectedPrompt && (
    draftName.trim() !== selectedPrompt.name ||
    draftHotkey.trim() !== selectedPrompt.hotkey ||
    draftText.trim() !== selectedPrompt.prompt_text ||
    draftMode !== selectedPrompt.mode
  );

  return (
    <SettingsGroup title={t("settings.customPrompts.title")}>
      <div className="space-y-3">
        {error && (
          <Alert variant="error" contained>
            {error}
          </Alert>
        )}

        <div className="flex gap-2">
          <Dropdown
            selectedValue={selectedId}
            options={prompts.map((p) => ({ value: p.id, label: p.name }))}
            onSelect={(value) => {
              setSelectedId(value);
              setIsCreating(false);
              setError(null);
            }}
            placeholder={
              hasPrompts
                ? t("settings.customPrompts.selectedPrompt")
                : t("settings.customPrompts.selectPrompt")
            }
            className="flex-1"
          />
          <Button
            onClick={handleStartCreate}
            variant="primary"
            size="md"
            disabled={isCreating}
          >
            {t("settings.customPrompts.createNew")}
          </Button>
        </div>

        {(isCreating || (selectedPrompt && !isCreating)) && (
          <div className="space-y-3">
            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.customPrompts.promptName")}
              </label>
              <Input
                type="text"
                value={draftName}
                onChange={(e) => setDraftName(e.target.value)}
                placeholder={t("settings.customPrompts.promptNamePlaceholder")}
                variant="compact"
              />
            </div>

            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.customPrompts.hotkey.title")}
              </label>
              <HotkeyPromptInput
                value={draftHotkey}
                onChange={setDraftHotkey}
              />
            </div>

            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.customPrompts.mode.title")}
              </label>
              <div className="flex gap-4">
                <label className="flex items-center gap-2 text-sm cursor-pointer">
                  <input
                    type="radio"
                    name="promptMode"
                    checked={draftMode === "selection"}
                    onChange={() => setDraftMode("selection")}
                    className="accent-logo-primary"
                  />
                  {t("settings.customPrompts.mode.selection")}
                </label>
                <label className="flex items-center gap-2 text-sm cursor-pointer">
                  <input
                    type="radio"
                    name="promptMode"
                    checked={draftMode === "voice"}
                    onChange={() => setDraftMode("voice")}
                    className="accent-logo-primary"
                  />
                  {t("settings.customPrompts.mode.voice")}
                </label>
              </div>
              <p className="text-xs text-mid-gray/70">
                {draftMode === "selection"
                  ? t("settings.customPrompts.mode.selectionDescription")
                  : t("settings.customPrompts.mode.voiceDescription")}
              </p>
            </div>

            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.customPrompts.promptInstructions")}
              </label>
              <Textarea
                value={draftText}
                onChange={(e) => setDraftText(e.target.value)}
                placeholder={t("settings.customPrompts.promptInstructionsPlaceholder")}
              />
              <p className="text-xs text-mid-gray/70">
                <Trans
                  i18nKey="settings.customPrompts.promptTip"
                  components={{ code: <code /> }}
                />
              </p>
            </div>

            <div className="flex gap-2 pt-2">
              {isCreating ? (
                <>
                  <Button
                    onClick={handleCreate}
                    variant="primary"
                    size="md"
                    disabled={!draftName.trim() || !draftHotkey.trim() || !draftText.trim() || saving}
                  >
                    {t("settings.customPrompts.createPrompt")}
                  </Button>
                  <Button
                    onClick={handleCancelCreate}
                    variant="secondary"
                    size="md"
                  >
                    {t("settings.customPrompts.cancel")}
                  </Button>
                </>
              ) : (
                <>
                  <Button
                    onClick={handleUpdate}
                    variant="primary"
                    size="md"
                    disabled={!draftName.trim() || !draftHotkey.trim() || !draftText.trim() || !isDirty || saving}
                  >
                    {t("settings.customPrompts.updatePrompt")}
                  </Button>
                  <Button
                    onClick={handleDeleteConfirm}
                    variant="danger"
                    size="md"
                    disabled={saving}
                  >
                    {t("settings.customPrompts.deletePrompt")}
                  </Button>
                </>
              )}
            </div>
          </div>
        )}

        {!isCreating && !selectedPrompt && (
          <div className="p-3 bg-mid-gray/5 rounded-md border border-mid-gray/20">
            <p className="text-sm text-mid-gray">
              {hasPrompts
                ? t("settings.customPrompts.selectToEdit")
                : t("settings.customPrompts.createFirst")}
            </p>
          </div>
        )}
      </div>
    </SettingsGroup>
  );
};
