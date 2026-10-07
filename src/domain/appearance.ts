export type AppTheme = "system" | "light" | "dark";
export interface AppearancePreferences {
  theme: AppTheme;
  lowPowerMode: boolean;
  fullscreenAutoHide: boolean;
}
export const defaultAppearance: AppearancePreferences = { theme: "dark", lowPowerMode: false, fullscreenAutoHide: true };
export function parseAppearance(value: string | null): AppearancePreferences {
  try {
    const stored = JSON.parse(value ?? "{}");
    return {
      theme: ["system", "light", "dark"].includes(stored?.theme) ? stored.theme : defaultAppearance.theme,
      lowPowerMode: stored?.lowPowerMode === true,
      fullscreenAutoHide: stored?.fullscreenAutoHide !== false
    };
  } catch { return { ...defaultAppearance }; }
}
