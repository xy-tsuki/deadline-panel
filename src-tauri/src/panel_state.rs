use serde::Serialize;

#[derive(Default)]
pub(crate) struct PanelState {
    pub manual_hidden: bool,
    pub auto_hidden: bool,
    pub expanded: bool,
    pub dragging: bool,
    pub open_down: bool,
    pub hide_token: u64,
    fullscreen_active: bool,
    show_until_fullscreen_exit: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PanelVisibility {
    pub visible: bool,
    pub manual_hidden: bool,
    pub auto_hidden: bool,
    pub expanded: bool,
    pub dragging: bool,
    pub direction: &'static str,
}

impl PanelState {
    pub fn visibility(&self) -> PanelVisibility {
        PanelVisibility {
            visible: !self.manual_hidden && !self.auto_hidden,
            manual_hidden: self.manual_hidden,
            auto_hidden: self.auto_hidden,
            expanded: self.expanded,
            dragging: self.dragging,
            direction: if self.open_down { "down" } else { "up" },
        }
    }

    pub fn show(&mut self) {
        self.hide_token = self.hide_token.wrapping_add(1);
        self.show_until_fullscreen_exit = self.fullscreen_active || self.auto_hidden;
        self.manual_hidden = false;
        self.auto_hidden = false;
        self.expanded = false;
        self.dragging = false;
    }

    pub fn hide_manually(&mut self) -> u64 {
        self.hide_token = self.hide_token.wrapping_add(1);
        self.manual_hidden = true;
        self.expanded = false;
        self.dragging = false;
        self.hide_token
    }

    pub fn expire_hide(&mut self, token: u64) -> bool {
        if token != self.hide_token {
            return false;
        }
        self.manual_hidden = false;
        self.expanded = false;
        true
    }

    // A tray "show" must win over the fullscreen app that caused the hide.
    // Resume automatic hiding only after that fullscreen session ends.
    pub fn observe_fullscreen(&mut self, fullscreen: bool) -> bool {
        self.fullscreen_active = fullscreen;
        if !fullscreen {
            self.show_until_fullscreen_exit = false;
        }
        let hidden = fullscreen && !self.show_until_fullscreen_exit;
        if hidden == self.auto_hidden {
            return false;
        }
        self.auto_hidden = hidden;
        if hidden {
            self.expanded = false;
            self.dragging = false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_show_cancels_old_timers_and_resets_interaction() {
        let mut state = PanelState::default();
        let token = state.hide_manually();
        state.dragging = true;
        state.expanded = true;
        state.show();
        assert!(state.visibility().visible);
        assert!(!state.dragging && !state.expanded);
        let new_token = state.hide_manually();
        assert!(!state.expire_hide(token));
        assert!(state.manual_hidden);
        assert!(state.expire_hide(new_token));
        assert!(state.visibility().visible);
    }

    #[test]
    fn explicit_show_wins_until_fullscreen_exits() {
        let mut state = PanelState::default();
        assert!(state.observe_fullscreen(true));
        assert!(!state.visibility().visible);
        state.show();
        for _ in 0..5 {
            assert!(!state.observe_fullscreen(true));
        }
        assert!(state.visibility().visible);
        assert!(!state.observe_fullscreen(false));
        assert!(state.observe_fullscreen(true));
        assert!(!state.visibility().visible);
        assert!(state.observe_fullscreen(false));
        assert!(state.visibility().visible);
    }

    #[test]
    fn fullscreen_exit_does_not_cancel_manual_hide() {
        let mut state = PanelState::default();
        state.observe_fullscreen(true);
        let token = state.hide_manually();
        state.observe_fullscreen(false);
        assert!(!state.visibility().visible);
        state.expire_hide(token);
        assert!(state.visibility().visible);
        state.observe_fullscreen(true);
        let token = state.hide_manually();
        state.expire_hide(token);
        assert!(!state.visibility().visible);
    }
}
