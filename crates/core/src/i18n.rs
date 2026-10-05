//! Interface text in English and Brazilian Portuguese, picked from the Windows language.

pub struct Text {
    pub tagline: &'static str,
    pub empty_title: &'static str,
    pub empty_body: &'static str,
    pub controllers_one: &'static str,
    pub controllers_many: &'static str,
    pub playing_as: &'static str,
    pub not_xinput: &'static str,
    pub waiting: &'static str,
    pub native: &'static str,
    pub unmapped: &'static str,
    pub unavailable: &'static str,
    pub hidden: &'static str,
    pub visible: &'static str,
    pub usb: &'static str,
    pub bluetooth: &'static str,
    pub dongle: &'static str,
    pub wireless: &'static str,
    pub charging: &'static str,
    pub charged: &'static str,
    pub wired_power: &'static str,
    pub turn_off: &'static str,
    pub hide_originals: &'static str,
    pub hide_originals_hint: &'static str,
    pub start_with_windows: &'static str,
    pub vigem_missing: &'static str,
    pub hidhide_missing: &'static str,
    pub download: &'static str,
    pub ready: &'static str,
    pub problem: &'static str,
    pub open: &'static str,
    pub quit: &'static str,
    pub quit_hint: &'static str,
    pub tray_off: &'static str,
}

pub const EN: Text = Text {
    tagline: "Any controller, any connection, as an Xbox controller.",
    empty_title: "Connect a controller",
    empty_body: "By cable, Bluetooth or its receiver. Any brand; several at once.",
    controllers_one: "1 controller",
    controllers_many: "controllers",
    playing_as: "Xbox controller",
    not_xinput: "Xbox controller (5th+, not visible to XInput games)",
    waiting: "Waiting for it to come back",
    native: "Xbox controller, read by games directly",
    unmapped: "No button layout known for this device",
    unavailable: "Not available to games: ViGEmBus is missing",
    hidden: "original hidden from games",
    visible: "original also visible to games",
    usb: "USB",
    bluetooth: "Bluetooth",
    dongle: "Receiver",
    wireless: "Wireless",
    charging: "charging",
    charged: "charged",
    wired_power: "cable",
    turn_off: "Turn off",
    hide_originals: "Hide the originals from games",
    hide_originals_hint: "Stops games that read PlayStation or Switch controllers from seeing them twice.",
    start_with_windows: "Start with Windows",
    vigem_missing: "ViGEmBus is not installed. Controllers are detected, but games cannot see them.",
    hidhide_missing: "HidHide is not installed. Games that read PlayStation or Switch controllers may see them twice.",
    download: "Download",
    ready: "Ready",
    problem: "Needs attention",
    open: "Open",
    quit: "Quit",
    quit_hint: "Closing this window keeps Open Controller running in the notification area.",
    tray_off: "Open Controller is not running. Start it to use your controllers.",
};

pub const PT: Text = Text {
    tagline: "Qualquer controle, qualquer conexão, como um controle de Xbox.",
    empty_title: "Conecte um controle",
    empty_body: "Por cabo, Bluetooth ou receptor. Qualquer marca; vários ao mesmo tempo.",
    controllers_one: "1 controle",
    controllers_many: "controles",
    playing_as: "Controle de Xbox",
    not_xinput: "Controle de Xbox (5º em diante, invisível para jogos XInput)",
    waiting: "Esperando ele voltar",
    native: "Controle de Xbox, lido direto pelos jogos",
    unmapped: "Nenhum layout de botões conhecido para este dispositivo",
    unavailable: "Indisponível para os jogos: falta o ViGEmBus",
    hidden: "original escondido dos jogos",
    visible: "original também visível para os jogos",
    usb: "USB",
    bluetooth: "Bluetooth",
    dongle: "Receptor",
    wireless: "Sem fio",
    charging: "carregando",
    charged: "carregado",
    wired_power: "cabo",
    turn_off: "Desligar",
    hide_originals: "Esconder os originais dos jogos",
    hide_originals_hint: "Evita que jogos que leem controles de PlayStation ou Switch os vejam duas vezes.",
    start_with_windows: "Iniciar com o Windows",
    vigem_missing: "O ViGEmBus não está instalado. Os controles são detectados, mas os jogos não os veem.",
    hidhide_missing: "O HidHide não está instalado. Jogos que leem controles de PlayStation ou Switch podem vê-los duas vezes.",
    download: "Baixar",
    ready: "Pronto",
    problem: "Precisa de atenção",
    open: "Abrir",
    quit: "Sair",
    quit_hint: "Fechar esta janela mantém o Open Controller rodando na área de notificação.",
    tray_off: "O Open Controller não está rodando. Inicie-o para usar seus controles.",
};

/// The text in the Windows display language: Portuguese or, for any other, English.
/// `OPEN_CONTROLLER_LANG=en` or `pt` overrides it.
pub fn text() -> &'static Text {
    match std::env::var("OPEN_CONTROLLER_LANG").as_deref() {
        Ok("en") => &EN,
        Ok("pt") => &PT,
        _ if portuguese() => &PT,
        _ => &EN,
    }
}

#[cfg(windows)]
fn portuguese() -> bool {
    const LANG_PORTUGUESE: u16 = 0x16;
    let lang = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    lang & 0x3FF == LANG_PORTUGUESE
}

#[cfg(not(windows))]
fn portuguese() -> bool {
    false
}
