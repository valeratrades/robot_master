use bevy::prelude::Color;

// Background surfaces
pub const BG_DARK: Color = Color::oklcha(0.15, 0.03, 260.0, 1.0); // deep navy
pub const BG_RESULT: Color = Color::oklcha(0.12, 0.03, 260.0, 1.0); // even darker

// Buttons
pub const BTN_NORMAL: Color = Color::oklcha(0.35, 0.06, 260.0, 0.8);
pub const BTN_HOVERED: Color = Color::oklcha(0.45, 0.08, 260.0, 0.9);
pub const BTN_PRESSED: Color = Color::oklcha(0.65, 0.15, 145.0, 0.9); // green flash
pub const BTN_START: Color = Color::oklcha(0.60, 0.15, 145.0, 0.8);
pub const BTN_START_HOVER: Color = Color::oklcha(0.70, 0.18, 145.0, 1.0);
pub const BTN_PLAY_AGAIN: Color = Color::oklcha(0.55, 0.12, 145.0, 0.8);
pub const BTN_PLAY_AGAIN_HOVER: Color = Color::oklcha(0.65, 0.15, 145.0, 1.0);

// Board cells
pub const CELL_OCCUPIED: Color = Color::oklcha(0.40, 0.06, 260.0, 0.8);
pub const CELL_EMPTY: Color = Color::oklcha(0.25, 0.03, 260.0, 0.4);
pub const CELL_HIGHLIGHT: Color = Color::oklcha(0.55, 0.15, 145.0, 0.5); // green for playable

// Hand cards
pub const HAND_CARD: Color = Color::oklcha(0.40, 0.06, 260.0, 0.7);
pub const HAND_CARD_EMPTY: Color = Color::oklcha(0.20, 0.01, 260.0, 0.3);
pub const HAND_CARD_SELECTED: Color = Color::oklcha(0.70, 0.15, 90.0, 0.8); // yellow
pub const HAND_CARD_HOVER: Color = Color::oklcha(0.50, 0.08, 260.0, 0.9);
pub const HAND_CARD_OPPONENT: Color = Color::oklcha(0.20, 0.02, 260.0, 0.5);

// Text
pub const TEXT_PRIMARY: Color = Color::WHITE;
pub const TEXT_TITLE: Color = Color::oklcha(0.85, 0.15, 90.0, 1.0); // gold
pub const TEXT_P1: Color = Color::oklcha(0.65, 0.15, 250.0, 1.0); // blue
pub const TEXT_P2: Color = Color::oklcha(0.65, 0.15, 25.0, 1.0); // red
pub const TEXT_LABEL: Color = Color::oklcha(0.75, 0.10, 90.0, 1.0); // warm yellow
pub const TEXT_MUTED: Color = Color::oklcha(0.50, 0.0, 0.0, 0.5);
pub const TEXT_SELECTION: Color = Color::oklcha(0.85, 0.18, 90.0, 1.0); // bright yellow
pub const TEXT_GAME_OVER: Color = Color::oklcha(0.65, 0.18, 25.0, 1.0); // red
pub const TEXT_ELO: Color = Color::oklcha(0.75, 0.10, 90.0, 0.9);

// Eval bar segments
pub const EVAL_BAR_P1: Color = Color::oklcha(0.55, 0.18, 250.0, 0.9); // blue
pub const EVAL_BAR_P2: Color = Color::oklcha(0.55, 0.18, 25.0, 0.9); // red
