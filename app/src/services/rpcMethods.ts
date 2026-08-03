export const CORE_RPC_METHODS = {
  configGet: 'aura.config_get',
  configGetAnalyticsSettings: 'aura.config_get_analytics_settings',
  configGetComposioTriggerSettings: 'aura.config_get_composio_trigger_settings',
  configGetRuntimeFlags: 'aura.config_get_runtime_flags',
  configSetBrowserAllowAll: 'aura.config_set_browser_allow_all',
  configUpdateAnalyticsSettings: 'aura.config_update_analytics_settings',
  configUpdateBrowserSettings: 'aura.config_update_browser_settings',
  configUpdateComposioTriggerSettings: 'aura.config_update_composio_trigger_settings',
  configUpdateLocalAiSettings: 'aura.config_update_local_ai_settings',
  configUpdateMemorySettings: 'aura.config_update_memory_settings',
  configUpdateModelSettings: 'aura.config_update_model_settings',
  configUpdateRuntimeSettings: 'aura.config_update_runtime_settings',
  configUpdateScreenIntelligenceSettings: 'aura.config_update_screen_intelligence_settings',
  configWorkspaceOnboardingFlagExists: 'aura.config_workspace_onboarding_flag_exists',
  configWorkspaceOnboardingFlagSet: 'aura.config_workspace_onboarding_flag_set',
  corePing: 'core.ping',
  inferenceApplyPreset: 'aura.inference_apply_preset',
  inferenceDiagnostics: 'aura.inference_diagnostics',
  inferenceDeviceProfile: 'aura.inference_device_profile',
  inferenceGetClientConfig: 'aura.inference_get_client_config',
  inferenceListModels: 'aura.inference_list_models',
  inferencePresets: 'aura.inference_presets',
  inferenceUpdateLocalSettings: 'aura.inference_update_local_settings',
  inferenceUpdateModelSettings: 'aura.inference_update_model_settings',
  providersListModels: 'aura.inference_list_models',
  screenIntelligenceStatus: 'aura.screen_intelligence_status',
} as const;

export type CoreRpcMethod = (typeof CORE_RPC_METHODS)[keyof typeof CORE_RPC_METHODS];

export const LEGACY_METHOD_ALIASES: Record<string, CoreRpcMethod> = {
  'aura.get_analytics_settings': CORE_RPC_METHODS.configGetAnalyticsSettings,
  'aura.get_composio_trigger_settings': CORE_RPC_METHODS.configGetComposioTriggerSettings,
  'aura.get_config': CORE_RPC_METHODS.configGet,
  'aura.get_runtime_flags': CORE_RPC_METHODS.configGetRuntimeFlags,
  'aura.ping': CORE_RPC_METHODS.corePing,
  'aura.set_browser_allow_all': CORE_RPC_METHODS.configSetBrowserAllowAll,
  'aura.update_analytics_settings': CORE_RPC_METHODS.configUpdateAnalyticsSettings,
  'aura.update_browser_settings': CORE_RPC_METHODS.configUpdateBrowserSettings,
  'aura.update_composio_trigger_settings':
    CORE_RPC_METHODS.configUpdateComposioTriggerSettings,
  'aura.update_local_ai_settings': CORE_RPC_METHODS.inferenceUpdateLocalSettings,
  'aura.update_memory_settings': CORE_RPC_METHODS.configUpdateMemorySettings,
  'aura.update_model_settings': CORE_RPC_METHODS.inferenceUpdateModelSettings,
  'aura.update_runtime_settings': CORE_RPC_METHODS.configUpdateRuntimeSettings,
  'aura.update_screen_intelligence_settings':
    CORE_RPC_METHODS.configUpdateScreenIntelligenceSettings,
  'aura.workspace_onboarding_flag_exists':
    CORE_RPC_METHODS.configWorkspaceOnboardingFlagExists,
  'aura.workspace_onboarding_flag_set': CORE_RPC_METHODS.configWorkspaceOnboardingFlagSet,
  'aura.local_ai_apply_preset': CORE_RPC_METHODS.inferenceApplyPreset,
  'aura.local_ai_device_profile': CORE_RPC_METHODS.inferenceDeviceProfile,
  'aura.local_ai_diagnostics': CORE_RPC_METHODS.inferenceDiagnostics,
  'aura.local_ai_presets': CORE_RPC_METHODS.inferencePresets,
  'aura.providers_list_models': CORE_RPC_METHODS.inferenceListModels,
};

export function normalizeRpcMethod(method: string): string {
  const normalized = method.trim().toLowerCase();

  if (normalized in LEGACY_METHOD_ALIASES) {
    return LEGACY_METHOD_ALIASES[normalized];
  }

  if (normalized.startsWith('aura.auth.')) {
    return `aura.auth_${normalized.slice('aura.auth.'.length).split('.').join('_')}`;
  }

  if (normalized.startsWith('aura.accessibility_')) {
    return normalized.replace('aura.accessibility_', 'aura.screen_intelligence_');
  }

  return normalized;
}
