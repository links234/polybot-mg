//! Market data display components

use crate::execution::orderbook::PriceLevel;
use egui::{Response, Ui};
use rust_decimal::Decimal;
use std::time::Instant;


/// Display streaming status
pub fn streaming_status(ui: &mut Ui, connected: bool, asset_count: usize) -> Response {
    ui.horizontal(|ui| {
        let status_color = if connected {
            egui::Color32::from_rgb(100, 200, 100)
        } else {
            egui::Color32::from_rgb(200, 100, 100)
        };

        let status_text = if connected {
            "🟢 Connected"
        } else {
            "🔴 Disconnected"
        };
        ui.colored_label(status_color, status_text);

        ui.separator();

        ui.label(format!("📡 Streaming {} assets", asset_count));
    })
    .response
}

/// Enhanced order book display with change flash animations
pub fn order_book_display_enhanced(
    ui: &mut Ui,
    bids: &[PriceLevel],
    asks: &[PriceLevel],
    changes: &[(Decimal, Decimal, Instant, bool)], // (price, size, changed_at, is_bid)
) -> Response {
    ui.vertical(|ui| {
        ui.heading("📊 Order Book");

        // Calculate spread
        let spread = if !bids.is_empty() && !asks.is_empty() {
            let best_bid = &bids[0];
            let best_ask = &asks[0];
            let spread_val = best_ask.price - best_bid.price;
            let spread_pct = (spread_val / best_ask.price) * Decimal::from(100);
            Some((spread_val, spread_pct))
        } else {
            None
        };

        // Show spread info
        if let Some((spread_val, spread_pct)) = spread {
            ui.horizontal(|ui| {
                ui.label("Spread:");
                ui.colored_label(
                    egui::Color32::from_rgb(150, 150, 150),
                    format!("${:.4} ({:.2}%)", spread_val, spread_pct),
                );
            });
        }

        ui.separator();

        // Order book grid
        egui::Grid::new("orderbook_grid_enhanced")
            .num_columns(4)
            .spacing([10.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                // Header
                ui.heading("Bid Size");
                ui.heading("Bid Price");
                ui.heading("Ask Price");
                ui.heading("Ask Size");
                ui.end_row();

                // Rows
                let max_rows = bids.len().max(asks.len()).min(15);
                let now = Instant::now();

                for i in 0..max_rows {
                    // Bid side
                    if i < bids.len() {
                        let bid = &bids[i];

                        // Check if this bid has recently changed
                        let is_changed = changes.iter().any(|(price, _, changed_at, is_bid)| {
                            *is_bid
                                && *price == bid.price
                                && now.duration_since(*changed_at).as_millis() < 1000
                        });

                        // Size label with flash effect
                        let size_response = ui.label(format!("{:.2}", bid.size));
                        if is_changed {
                            // Calculate flash intensity based on how recent the change is
                            let age_ms = changes
                                .iter()
                                .filter(|(price, _, _, is_bid)| *is_bid && *price == bid.price)
                                .map(|(_, _, changed_at, _)| {
                                    now.duration_since(*changed_at).as_millis()
                                })
                                .min()
                                .unwrap_or(1000);

                            let flash_intensity = 1.0 - (age_ms as f32 / 1000.0);
                            let flash_color = egui::Color32::from_rgba_unmultiplied(
                                100,
                                255,
                                100,
                                (flash_intensity * 60.0) as u8,
                            );

                            // Draw flash background for size cell
                            ui.painter().rect_filled(
                                size_response.rect.expand(2.0),
                                2.0,
                                flash_color,
                            );
                        }

                        // Price label with flash effect
                        let price_response = ui.colored_label(
                            egui::Color32::from_rgb(100, 200, 100),
                            format!("${:.4}", bid.price),
                        );
                        if is_changed {
                            let age_ms = changes
                                .iter()
                                .filter(|(price, _, _, is_bid)| *is_bid && *price == bid.price)
                                .map(|(_, _, changed_at, _)| {
                                    now.duration_since(*changed_at).as_millis()
                                })
                                .min()
                                .unwrap_or(1000);

                            let flash_intensity = 1.0 - (age_ms as f32 / 1000.0);
                            let flash_color = egui::Color32::from_rgba_unmultiplied(
                                100,
                                255,
                                100,
                                (flash_intensity * 60.0) as u8,
                            );

                            // Draw flash background for price cell
                            ui.painter().rect_filled(
                                price_response.rect.expand(2.0),
                                2.0,
                                flash_color,
                            );
                        }
                    } else {
                        ui.label("");
                        ui.label("");
                    }

                    // Ask side
                    if i < asks.len() {
                        let ask = &asks[i];

                        // Check if this ask has recently changed
                        let is_changed = changes.iter().any(|(price, _, changed_at, is_bid)| {
                            !*is_bid
                                && *price == ask.price
                                && now.duration_since(*changed_at).as_millis() < 1000
                        });

                        // Price label with flash effect
                        let price_response = ui.colored_label(
                            egui::Color32::from_rgb(200, 100, 100),
                            format!("${:.4}", ask.price),
                        );
                        if is_changed {
                            let age_ms = changes
                                .iter()
                                .filter(|(price, _, _, is_bid)| !*is_bid && *price == ask.price)
                                .map(|(_, _, changed_at, _)| {
                                    now.duration_since(*changed_at).as_millis()
                                })
                                .min()
                                .unwrap_or(1000);

                            let flash_intensity = 1.0 - (age_ms as f32 / 1000.0);
                            let flash_color = egui::Color32::from_rgba_unmultiplied(
                                255,
                                100,
                                100,
                                (flash_intensity * 60.0) as u8,
                            );

                            // Draw flash background for price cell
                            ui.painter().rect_filled(
                                price_response.rect.expand(2.0),
                                2.0,
                                flash_color,
                            );
                        }

                        // Size label with flash effect
                        let size_response = ui.label(format!("{:.2}", ask.size));
                        if is_changed {
                            let age_ms = changes
                                .iter()
                                .filter(|(price, _, _, is_bid)| !*is_bid && *price == ask.price)
                                .map(|(_, _, changed_at, _)| {
                                    now.duration_since(*changed_at).as_millis()
                                })
                                .min()
                                .unwrap_or(1000);

                            let flash_intensity = 1.0 - (age_ms as f32 / 1000.0);
                            let flash_color = egui::Color32::from_rgba_unmultiplied(
                                255,
                                100,
                                100,
                                (flash_intensity * 60.0) as u8,
                            );

                            // Draw flash background for size cell
                            ui.painter().rect_filled(
                                size_response.rect.expand(2.0),
                                2.0,
                                flash_color,
                            );
                        }
                    } else {
                        ui.label("");
                        ui.label("");
                    }

                    ui.end_row();
                }
            });
    })
    .response
}
