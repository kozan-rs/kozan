//! Keyboard event types — W3C UIEvents specification.
//!
//! This module contains material derived from the W3C UIEvents specification:
//! - `KeyCode`: <https://www.w3.org/TR/2017/CR-uievents-code-20170601/>
//! - `NamedKey`: <https://www.w3.org/TR/2017/CR-uievents-key-20170601/>
//!
//! Copyright (c) 2017 W3C (MIT, ERCIM, Keio, Beihang).
//! Licensed under <http://www.w3.org/Consortium/Legal/copyright-software>.

use std::time::Instant;

use super::modifiers::Modifiers;
use super::mouse::ButtonState;

/// Keyboard key press or release event.
#[derive(Debug, Clone)]
pub struct KeyboardEvent {
    /// Physical key code — which key on the keyboard (layout-independent).
    pub physical_key: KeyCode,
    /// Logical key value — what the key means (layout-dependent).
    pub logical_key: Key,
    pub state: ButtonState,
    pub modifiers: Modifiers,
    /// Where the key is on the keyboard (standard, left, right, numpad).
    pub location: KeyLocation,
    /// Text input produced by this key press (if any).
    pub text: Option<String>,
    /// Whether this is an auto-repeat event (key held down).
    pub repeat: bool,
    pub timestamp: Instant,
}

/// Physical key code — identifies which key on the keyboard by position.
///
/// Follows W3C UIEvents `KeyboardEvent.code`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyCode {
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Equal,
    /// Located between the left Shift and Z keys. Labeled `\` on a UK keyboard.
    IntlBackslash,
    /// Located between `/` and right Shift. Labeled `\` (ro) on a Japanese keyboard.
    IntlRo,
    /// Located between `=` and Backspace. Labeled `¥` (yen) on a Japanese keyboard.
    IntlYen,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    AltLeft,
    AltRight,
    Backspace,
    CapsLock,
    ContextMenu,
    ControlLeft,
    ControlRight,
    Enter,
    SuperLeft,
    SuperRight,
    ShiftLeft,
    ShiftRight,
    Space,
    Tab,
    /// Japanese: henkan
    Convert,
    /// Japanese: katakana/hiragana/romaji
    KanaMode,
    /// Korean: HangulMode (han/yeong). Japanese Mac: kana.
    Lang1,
    /// Korean: Hanja (hanja). Japanese Mac: eisu.
    Lang2,
    /// Japanese word-processing keyboard: Katakana
    Lang3,
    /// Japanese word-processing keyboard: Hiragana
    Lang4,
    /// Japanese word-processing keyboard: Zenkaku/Hankaku
    Lang5,
    /// Japanese: muhenkan
    NonConvert,
    Delete,
    End,
    Help,
    Home,
    Insert,
    PageDown,
    PageUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadBackspace,
    NumpadClear,
    NumpadClearEntry,
    NumpadComma,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadEqual,
    NumpadHash,
    NumpadMemoryAdd,
    NumpadMemoryClear,
    NumpadMemoryRecall,
    NumpadMemoryStore,
    NumpadMemorySubtract,
    NumpadMultiply,
    NumpadParenLeft,
    NumpadParenRight,
    NumpadStar,
    NumpadSubtract,
    Escape,
    Fn,
    FnLock,
    PrintScreen,
    ScrollLock,
    Pause,
    BrowserBack,
    BrowserFavorites,
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    Eject,
    LaunchApp1,
    LaunchApp2,
    LaunchMail,
    MediaPlayPause,
    MediaSelect,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    Power,
    Sleep,
    AudioVolumeDown,
    AudioVolumeMute,
    AudioVolumeUp,
    WakeUp,
    Meta,
    Hyper,
    Turbo,
    Abort,
    Resume,
    Suspend,
    Again,
    Copy,
    Cut,
    Find,
    Open,
    Paste,
    Props,
    Select,
    Undo,
    /// Dedicated hiragana key on some Japanese word processing keyboards.
    Hiragana,
    /// Dedicated katakana key on some Japanese word processing keyboards.
    Katakana,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
    /// Unrecognized key — no W3C code mapping available.
    Unidentified,
}

/// Logical named key — W3C `KeyboardEvent.key` named values.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NamedKey {
    Alt,
    AltGraph,
    CapsLock,
    Control,
    Fn,
    FnLock,
    NumLock,
    ScrollLock,
    Shift,
    Symbol,
    SymbolLock,
    Meta,
    Hyper,
    Super,
    Enter,
    Tab,
    Space,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    End,
    Home,
    PageDown,
    PageUp,
    Backspace,
    Clear,
    Copy,
    CrSel,
    Cut,
    Delete,
    EraseEof,
    ExSel,
    Insert,
    Paste,
    Redo,
    Undo,
    Accept,
    Again,
    Attn,
    Cancel,
    ContextMenu,
    Escape,
    Execute,
    Find,
    Help,
    Pause,
    Play,
    Props,
    Select,
    ZoomIn,
    ZoomOut,
    BrightnessDown,
    BrightnessUp,
    Eject,
    LogOff,
    Power,
    PowerOff,
    PrintScreen,
    Hibernate,
    Standby,
    WakeUp,
    AllCandidates,
    Alphanumeric,
    CodeInput,
    Compose,
    Convert,
    FinalMode,
    GroupFirst,
    GroupLast,
    GroupNext,
    GroupPrevious,
    ModeChange,
    NextCandidate,
    NonConvert,
    PreviousCandidate,
    Process,
    SingleCandidate,
    HangulMode,
    HanjaMode,
    JunjaMode,
    Eisu,
    Hankaku,
    Hiragana,
    HiraganaKatakana,
    KanaMode,
    KanjiMode,
    Katakana,
    Romaji,
    Zenkaku,
    ZenkakuHankaku,
    Soft1,
    Soft2,
    Soft3,
    Soft4,
    ChannelDown,
    ChannelUp,
    Close,
    MailForward,
    MailReply,
    MailSend,
    MediaClose,
    MediaFastForward,
    MediaPause,
    MediaPlay,
    MediaPlayPause,
    MediaRecord,
    MediaRewind,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    New,
    Open,
    Print,
    Save,
    SpellCheck,
    Key11,
    Key12,
    AudioBalanceLeft,
    AudioBalanceRight,
    AudioBassBoostDown,
    AudioBassBoostToggle,
    AudioBassBoostUp,
    AudioFaderFront,
    AudioFaderRear,
    AudioSurroundModeNext,
    AudioTrebleDown,
    AudioTrebleUp,
    AudioVolumeDown,
    AudioVolumeUp,
    AudioVolumeMute,
    MicrophoneToggle,
    MicrophoneVolumeDown,
    MicrophoneVolumeUp,
    MicrophoneVolumeMute,
    SpeechCorrectionList,
    SpeechInputToggle,
    LaunchApplication1,
    LaunchApplication2,
    LaunchCalendar,
    LaunchContacts,
    LaunchMail,
    LaunchMediaPlayer,
    LaunchMusicPlayer,
    LaunchPhone,
    LaunchScreenSaver,
    LaunchSpreadsheet,
    LaunchWebBrowser,
    LaunchWebCam,
    LaunchWordProcessor,
    BrowserBack,
    BrowserFavorites,
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    AppSwitch,
    Call,
    Camera,
    CameraFocus,
    EndCall,
    GoBack,
    GoHome,
    HeadsetHook,
    LastNumberRedial,
    Notification,
    MannerMode,
    VoiceDial,
    TV,
    TV3DMode,
    TVAntennaCable,
    TVAudioDescription,
    TVAudioDescriptionMixDown,
    TVAudioDescriptionMixUp,
    TVContentsMenu,
    TVDataService,
    TVInput,
    TVInputComponent1,
    TVInputComponent2,
    TVInputComposite1,
    TVInputComposite2,
    TVInputHDMI1,
    TVInputHDMI2,
    TVInputHDMI3,
    TVInputHDMI4,
    TVInputVGA1,
    TVMediaContext,
    TVNetwork,
    TVNumberEntry,
    TVPower,
    TVRadioService,
    TVSatellite,
    TVSatelliteBS,
    TVSatelliteCS,
    TVSatelliteToggle,
    TVTerrestrialAnalog,
    TVTerrestrialDigital,
    TVTimer,
    AVRInput,
    AVRPower,
    ColorF0Red,
    ColorF1Green,
    ColorF2Yellow,
    ColorF3Blue,
    ColorF4Grey,
    ColorF5Brown,
    ClosedCaptionToggle,
    Dimmer,
    DisplaySwap,
    DVR,
    Exit,
    FavoriteClear0,
    FavoriteClear1,
    FavoriteClear2,
    FavoriteClear3,
    FavoriteRecall0,
    FavoriteRecall1,
    FavoriteRecall2,
    FavoriteRecall3,
    FavoriteStore0,
    FavoriteStore1,
    FavoriteStore2,
    FavoriteStore3,
    Guide,
    GuideNextDay,
    GuidePreviousDay,
    Info,
    InstantReplay,
    Link,
    ListProgram,
    LiveContent,
    Lock,
    MediaApps,
    MediaAudioTrack,
    MediaLast,
    MediaSkipBackward,
    MediaSkipForward,
    MediaStepBackward,
    MediaStepForward,
    MediaTopMenu,
    NavigateIn,
    NavigateNext,
    NavigateOut,
    NavigatePrevious,
    NextFavoriteChannel,
    NextUserProfile,
    OnDemand,
    Pairing,
    PinPDown,
    PinPMove,
    PinPToggle,
    PinPUp,
    PlaySpeedDown,
    PlaySpeedReset,
    PlaySpeedUp,
    RandomToggle,
    RcLowBattery,
    RecordSpeedNext,
    RfBypass,
    ScanChannelsToggle,
    ScreenModeNext,
    Settings,
    SplitScreenToggle,
    STBInput,
    STBPower,
    Subtitle,
    Teletext,
    VideoModeNext,
    Wink,
    ZoomToggle,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
}

/// The logical key value — what a key press means.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key<Str = String> {
    Named(NamedKey),
    Character(Str),
    Unidentified,
    Dead(Option<char>),
}

impl Key<String> {
    /// Convert `Key<String>` to `Key<&str>` for ergonomic matching.
    #[must_use]
    pub fn as_ref(&self) -> Key<&str> {
        match self {
            Key::Named(a) => Key::Named(*a),
            Key::Character(ch) => Key::Character(ch.as_str()),
            Key::Dead(d) => Key::Dead(*d),
            Key::Unidentified => Key::Unidentified,
        }
    }
}

impl From<NamedKey> for Key {
    #[inline]
    fn from(named: NamedKey) -> Self {
        Key::Named(named)
    }
}

impl<Str: PartialEq<str>> PartialEq<str> for Key<Str> {
    #[inline]
    fn eq(&self, rhs: &str) -> bool {
        match self {
            Key::Character(s) => s == rhs,
            _ => false,
        }
    }
}

impl<Str: PartialEq<str>> PartialEq<&str> for Key<Str> {
    #[inline]
    fn eq(&self, rhs: &&str) -> bool {
        self == *rhs
    }
}

impl<Str> PartialEq<NamedKey> for Key<Str> {
    #[inline]
    fn eq(&self, rhs: &NamedKey) -> bool {
        match self {
            Key::Named(a) => a == rhs,
            _ => false,
        }
    }
}

/// Where the key is physically located on the keyboard.
///
/// W3C `KeyboardEvent.location`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyLocation {
    #[default]
    Standard,
    Left,
    Right,
    Numpad,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_event_with_text() {
        let evt = KeyboardEvent {
            physical_key: KeyCode::KeyA,
            logical_key: Key::Character("a".to_string()),
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY,
            location: KeyLocation::Standard,
            text: Some("a".to_string()),
            repeat: false,
            timestamp: Instant::now(),
        };
        assert_eq!(evt.physical_key, KeyCode::KeyA);
        assert_eq!(evt.text.as_deref(), Some("a"));
    }

    #[test]
    fn keyboard_event_repeat() {
        let evt = KeyboardEvent {
            physical_key: KeyCode::KeyA,
            logical_key: Key::Character("a".to_string()),
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY,
            location: KeyLocation::Standard,
            text: Some("a".to_string()),
            repeat: true,
            timestamp: Instant::now(),
        };
        assert!(evt.repeat);
    }

    #[test]
    fn keyboard_event_with_modifiers() {
        let evt = KeyboardEvent {
            physical_key: KeyCode::KeyC,
            logical_key: Key::Character("c".to_string()),
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY.with_ctrl(),
            location: KeyLocation::Standard,
            text: None,
            repeat: false,
            timestamp: Instant::now(),
        };
        assert!(evt.modifiers.ctrl());
        assert!(evt.text.is_none());
    }

    #[test]
    fn modifier_key_has_no_text() {
        let evt = KeyboardEvent {
            physical_key: KeyCode::ShiftLeft,
            logical_key: Key::Named(NamedKey::Shift),
            state: ButtonState::Pressed,
            modifiers: Modifiers::EMPTY.with_shift(),
            location: KeyLocation::Left,
            text: None,
            repeat: false,
            timestamp: Instant::now(),
        };
        assert_eq!(evt.physical_key, KeyCode::ShiftLeft);
        assert!(evt.text.is_none());
    }

    #[test]
    fn key_as_ref_conversion() {
        let key = Key::Character("a".to_string());
        assert_eq!(key.as_ref(), Key::Character("a"));
    }

    #[test]
    fn key_partial_eq_str() {
        let key = Key::Character("a".to_string());
        assert!(key == "a");
        assert!(key != "b");
    }

    #[test]
    fn key_partial_eq_named() {
        let key: Key = Key::Named(NamedKey::Enter);
        assert!(key == NamedKey::Enter);
        assert!(key != NamedKey::Tab);
    }
}
