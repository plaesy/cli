// Dihasilkan dari design-ir/1 `dsg_order_desk` (ADR 0010). Jangan diedit manual.
#![allow(non_snake_case)]

use dioxus::prelude::*;

#[component]
pub fn CpButton() -> Element {
    rsx! {
        div {
            "data-plaesy-id": "nd_button_root",
            "data-plaesy-kind": "frame",
            style: "background-color: #2563EB; color: #FFFFFF; border-radius: 8px",
            span {
                "data-plaesy-id": "nd_button_label",
                "data-plaesy-kind": "text",
                style: "color: #FFFFFF",
                { "Label" }
            }
        }
    }
}

#[component]
pub fn PgOrders() -> Element {
    rsx! {
        div {
            "data-plaesy-id": "nd_orders_frame",
            "data-plaesy-kind": "frame",
            style: "background-color: #FFFFFF",
            span {
                "data-plaesy-id": "nd_title",
                "data-plaesy-kind": "text",
                style: "color: #111827",
                { "Total pesanan per pelanggan" }
            }
            div {
                "data-plaesy-id": "nd_cancel",
                "data-plaesy-kind": "instance",
                "data-plaesy-component": "cp_button",
                "data-plaesy-variant-tone": "danger",
                "data-plaesy-variant-size": "md",
            }
        }
    }
}
