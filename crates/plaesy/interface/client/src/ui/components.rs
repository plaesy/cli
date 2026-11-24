//! Komponen UI reusable bergaya shadcn/ui untuk Dioxus 0.7.
//! Mirror kode dari komponen desain di Plaesy.figcraft (NavigationItem, Sidebar,
//! ReceiptPanel, CreateForm, ReviewCard, MobileSheet, ConfirmDialog) agar dapat
//! dipakai langsung di klien web Plaesy. Nama komponen, properti, dan design
//! token sinkron 1:1 dengan node di FigCraft agar implementasi tidak tertinggal
//! dari referensi desain.

use dioxus::prelude::*;
use dioxus_shadcn::{
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonVariant},
    card::{Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle},
    dialog::{DialogClose, DialogContent, DialogDescription, DialogOverlay, DialogTitle},
    input::Input,
    label::Label,
    scroll_area::ScrollArea,
    sidebar::{
        Sidebar as ShadcnSidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarItem,
    },
};

/// NavigationItem — mirror komponen NavigationItem di FigCraft (ID 4340).
/// Item navigasi untuk sidebar dengan indikator aktif dan optional onclick.
#[component]
pub fn NavigationItem(
    label: &'static str,
    #[props(default)] active: bool,
    #[props(default)] href: String,
    #[props(default)] onclick: Option<EventHandler<MouseEvent>>,
) -> Element {
    rsx! {
        SidebarItem {
            active,
            href,
            onclick,
            div {
                class: "flex h-3.5 w-3.5 shrink-0 items-center justify-center",
                div { class: "h-2 w-2 rounded-full bg-muted" }
            }
            span { class: "select-none", {label} }
        }
    }
}

/// Sidebar — mirror komponen Sidebar di FigCraft (ID 4376).
/// Shell navigasi utama: header, grup item navigasi (lewat children), dan footer.
/// Reusable: isi grup diatur oleh pemanggil melalui children.
#[component]
pub fn Sidebar(
    #[props(default)] collapsed: bool,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    rsx! {
        ShadcnSidebar {
            collapsed,
            class,
            "data-side": "left",
            SidebarHeader {
                class: "flex h-12 items-center gap-2 px-4",
                span { class: "text-xl font-bold", "Plaesy" }
            }
            SidebarGroup {
                {children}
            }
            SidebarFooter {
                class: "flex h-12 items-center justify-between px-4",
                Button {
                    variant: ButtonVariant::Outline,
                    class: "shrink-0",
                    "Settings"
                }
            }
        }
    }
}

/// ReceiptPanel — mirror komponen ReceiptPanel di FigCraft (ID 4344).
/// Panel konfirmasi/ringkasan dengan judul, deskripsi, konten, dan dua tombol aksi.
#[component]
pub fn ReceiptPanel(
    title: &'static str,
    description: &'static str,
    #[props(default)] confirm_label: &'static str,
    #[props(default)] cancel_label: &'static str,
    #[props(default)] on_confirm: Option<EventHandler<MouseEvent>>,
    #[props(default)] on_cancel: Option<EventHandler<MouseEvent>>,
    children: Element,
) -> Element {
    rsx! {
        Card {
            class: "w-full max-w-md",
            CardHeader {
                CardTitle { {title} }
                CardDescription { {description} }
            }
            CardContent {
                {children}
            }
            CardFooter {
                class: "flex justify-end gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: on_cancel.is_none(),
                    onclick: move |evt: MouseEvent| {
                        if let Some(handler) = on_cancel.as_ref() {
                            handler.call(evt);
                        }
                    },
                    {cancel_label}
                }
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: on_confirm.is_none(),
                    onclick: move |evt: MouseEvent| {
                        if let Some(handler) = on_confirm.as_ref() {
                            handler.call(evt);
                        }
                    },
                    {confirm_label}
                }
            }
        }
    }
}

/// ReviewCard — mirror komponen ReviewCard di FigCraft (ID 4360).
/// Kartu tinjauan dengan status badge, judul, deskripsi, dan dua tombol aksi.
#[component]
pub fn ReviewCard(
    status: &'static str,
    title: &'static str,
    description: &'static str,
    #[props(default)] on_approve: Option<EventHandler<MouseEvent>>,
    #[props(default)] on_reject: Option<EventHandler<MouseEvent>>,
) -> Element {
    let variant = match status {
        "pending" => BadgeVariant::Secondary,
        "approved" => BadgeVariant::Default,
        "rejected" => BadgeVariant::Destructive,
        _ => BadgeVariant::Outline,
    };

    rsx! {
        Card {
            class: "w-full max-w-sm",
            CardHeader {
                class: "flex flex-row items-center justify-between space-y-0 pb-2",
                CardTitle { {title} }
                Badge {
                    variant,
                    {status}
                }
            }
            CardDescription { {description} }
            CardFooter {
                class: "flex justify-end gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: on_reject.is_none(),
                    onclick: move |evt: MouseEvent| {
                        if let Some(handler) = on_reject.as_ref() {
                            handler.call(evt);
                        }
                    },
                    "Tolak"
                }
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: on_approve.is_none(),
                    onclick: move |evt: MouseEvent| {
                        if let Some(handler) = on_approve.as_ref() {
                            handler.call(evt);
                        }
                    },
                    "Setuju"
                }
            }
        }
    }
}

/// CreateForm — mirror komponen CreateForm di FigCraft (ID 4351).
/// Form pembuatan dengan judul, deskripsi, dua bidang input, dan tombol
/// Submit/Cancel. Setiap input exposed sebagai `value` + `on_value_change`
/// sehingga state dikendalikan oleh pemanggil (controlled).
#[component]
pub fn CreateForm(
    title: &'static str,
    #[props(default)] description: &'static str,
    #[props(default)] field1_label: &'static str,
    #[props(default)] field1_value: String,
    #[props(default)] field2_label: &'static str,
    #[props(default)] field2_value: String,
    #[props(default)] submit_label: &'static str,
    #[props(default)] cancel_label: &'static str,
    #[props(default)] on_submit: Option<EventHandler<MouseEvent>>,
    #[props(default)] on_cancel: Option<EventHandler<MouseEvent>>,
    #[props(default)] on_value_change1: Option<EventHandler<String>>,
    #[props(default)] on_value_change2: Option<EventHandler<String>>,
) -> Element {
    rsx! {
        Card {
            class: "w-full max-w-lg",
            CardHeader {
                CardTitle { {title} }
                CardDescription { {description} }
            }
            CardContent {
                class: "space-y-4",
                div { class: "space-y-1",
                    Label { {field1_label} }
                    Input {
                        value: field1_value,
                        placeholder: field1_label,
                        on_value_change: on_value_change1,
                    }
                }
                div { class: "space-y-1",
                    Label { {field2_label} }
                    Input {
                        value: field2_value,
                        placeholder: field2_label,
                        on_value_change: on_value_change2,
                    }
                }
            }
            CardFooter {
                class: "flex justify-end gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: on_cancel.is_none(),
                    onclick: move |evt: MouseEvent| {
                        if let Some(handler) = on_cancel.as_ref() {
                            handler.call(evt);
                        }
                    },
                    {cancel_label}
                }
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: on_submit.is_none(),
                    onclick: move |evt: MouseEvent| {
                        if let Some(handler) = on_submit.as_ref() {
                            handler.call(evt);
                        }
                    },
                    {submit_label}
                }
            }
        }
    }
}

/// MobileSheet — mirror komponen MobileSheet di FigCraft (ID 4367).
/// Sheet layar mobile: judul, area konten scrollable (lewat children), dan
/// penutup di bawah. Dibuka dengan `open: true`.
#[component]
pub fn MobileSheet(
    title: &'static str,
    #[props(default)] open: bool,
    #[props(default)] on_close: Option<EventHandler<bool>>,
    children: Element,
) -> Element {
    rsx! {
        div {
            DialogOverlay {
                open,
                on_open_change: on_close,
            }
            DialogContent {
                open,
                on_open_change: on_close,
                class: "fixed bottom-0 left-0 right-0 z-50 grid w-full max-h-[90vh] rounded-t-xl border border-border bg-background p-6 shadow-xl",
                DialogTitle { {title} }
                ScrollArea {
                    class: "flex-1 overflow-y-auto p-4",
                    {children}
                }
                div {
                    class: "h-12 border-t flex items-center justify-center bg-muted",
                    span { class: "text-xs text-muted-foreground", "Bottom Nav Placeholder" }
                }
            }
        }
    }
}

/// ConfirmDialog — dialog konfirmasi generik.
/// Overlay + konten modal dengan judul, deskripsi, tombol Tutup, dan dua aksi.
#[component]
pub fn ConfirmDialog(
    title: &'static str,
    description: &'static str,
    #[props(default)] open: bool,
    #[props(default)] on_close: Option<EventHandler<bool>>,
    #[props(default)] confirm_label: &'static str,
    #[props(default)] cancel_label: &'static str,
    #[props(default)] on_confirm: Option<EventHandler<MouseEvent>>,
    #[props(default)] on_cancel: Option<EventHandler<MouseEvent>>,
) -> Element {
    rsx! {
        div {
            DialogOverlay {
                open,
                on_open_change: on_close,
            }
            DialogContent {
                open,
                on_open_change: on_close,
                DialogTitle { {title} }
                DialogDescription { {description} }
                DialogClose {
                    on_open_change: on_close,
                    "×"
                }
                CardFooter {
                    class: "flex justify-end gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: on_cancel.is_none(),
                        onclick: move |evt: MouseEvent| {
                            if let Some(handler) = on_cancel.as_ref() {
                                handler.call(evt);
                            }
                        },
                        {cancel_label}
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: on_confirm.is_none(),
                        onclick: move |evt: MouseEvent| {
                            if let Some(handler) = on_confirm.as_ref() {
                                handler.call(evt);
                            }
                        },
                        {confirm_label}
                    }
                }
            }
        }
    }
}
