use super::*;

/// Exercise actual input, submit, cancellation and fill controls in isolated
/// native windows. The live application's broker is restored after this check.
#[cfg(feature = "test-support")]
pub(crate) fn smoke_demo_orders(cx: &mut HeadlessAppContext) -> anyhow::Result<()> {
    use anyhow::ensure;
    use gpui_kit::test::TestWindowExt;

    let previous = cx.update(|cx| {
        let previous = cx.global::<DemoBroker>().clone();
        cx.set_global(DemoBroker::default());
        previous
    });
    let mut handles = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        let mut open = |kind| -> anyhow::Result<(AnyWindowHandle, Entity<PanelContent>)> {
            let result = cx.update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(50.), px(50.)),
                            size(px(800.), px(440.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            PanelContent::new(kind, Symbol::Aapl, Period::Intraday, window, cx)
                        })
                    },
                )
            })?;
            handles.push(result.0);
            Ok(result)
        };
        let (entry, entry_view) = open(PanelKind::OrderEntry)?;
        let (orders, _) = open(PanelKind::Orders)?;
        let (positions, _) = open(PanelKind::Positions)?;
        for handle in &handles {
            cx.update_window(*handle, |_, window, cx| window.render_frame(cx))?;
        }

        // Invalid quantity must produce visible feedback, with no new order.
        cx.update_window(entry, |_, window, cx| {
            window.click("order-price", cx);
            window.press("ctrl-a", cx);
            window.input("340.00", cx);
            window.click("order-quantity", cx);
            window.press("ctrl-a", cx);
            window.input("0", cx);
            window.click("submit-order", cx);
            ensure!(
                window.find("order-feedback").visible(),
                "Invalid quantity has no visible feedback"
            );
            ensure!(
                cx.global::<DemoBroker>().orders.len() == 1,
                "Invalid quantity created an order"
            );
            ensure!(
                entry_view
                    .read(cx)
                    .feedback
                    .as_ref()
                    .is_some_and(|(_, valid)| !valid),
                "Invalid quantity was accepted"
            );
            window.click("order-quantity", cx);
            window.press("ctrl-a", cx);
            window.input("10", cx);
            window.click("submit-order", cx);
            ensure!(
                cx.global::<DemoBroker>().orders.len() == 2,
                "Submit did not record a pending order"
            );
            ensure!(
                cx.global::<DemoBroker>().available_cash(Symbol::Aapl) == 96_600.,
                "Pending order did not reserve cash"
            );
            Ok::<_, anyhow::Error>(())
        })??;

        cx.update_window(orders, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            window.click("cancel-1002", cx);
            ensure!(
                cx.global::<DemoBroker>().orders[1].status == OrderStatus::Cancelled,
                "Cancel button did not cancel the order"
            );
            ensure!(
                cx.global::<DemoBroker>().available_cash(Symbol::Aapl) == 100_000.,
                "Cancellation did not release reserved cash"
            );
            Ok::<_, anyhow::Error>(())
        })??;

        cx.update_window(entry, |_, window, cx| window.click("submit-order", cx))?;
        cx.update_window(orders, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            window.click("fill-1003", cx);
            ensure!(
                cx.global::<DemoBroker>().available_shares(Symbol::Aapl) == 110,
                "Buy fill did not update the holding"
            );
            ensure!(
                cx.global::<DemoBroker>().usd == 96_600.,
                "Buy fill did not debit cash"
            );
            Ok::<_, anyhow::Error>(())
        })??;

        cx.update_window(positions, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            ensure!(
                window.find("position-AAPL").visible(),
                "Updated AAPL holding is not visible"
            );
            Ok::<_, anyhow::Error>(())
        })??;

        cx.update_window(entry, |_, window, cx| {
            window.click("order-sell", cx);
            window.click("submit-order", cx);
        })?;
        cx.update_window(orders, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            window.click("fill-1004", cx);
            ensure!(
                cx.global::<DemoBroker>().available_shares(Symbol::Aapl) == 100,
                "Sell fill did not reduce the holding"
            );
            ensure!(
                cx.global::<DemoBroker>().usd == 100_000.,
                "Sell fill did not credit cash"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        println!(
            "PASS: native order input validation, pending buy, cancel, buy fill, sell fill and shared positions/cash"
        );
        Ok(())
    })();
    for handle in handles {
        let _ = cx.update_window(handle, |_, window, _| window.remove_window());
    }
    cx.update(|cx| cx.set_global(previous));
    result
}

/// Exercise the reference panel controls through real GPUI input events.
#[cfg(feature = "test-support")]
pub(crate) fn smoke_market_panels(cx: &mut HeadlessAppContext) -> anyhow::Result<()> {
    smoke_panel_state(cx)?;
    use anyhow::ensure;
    use gpui_kit::test::TestWindowExt;
    let mut handles = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        let mut open = |kind| -> anyhow::Result<(AnyWindowHandle, Entity<PanelContent>)> {
            let result = cx.update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(50.), px(50.)),
                            size(px(1100.), px(560.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            PanelContent::new(kind, Symbol::Aapl, Period::Intraday, window, cx)
                        })
                    },
                )
            })?;
            handles.push(result.0);
            Ok(result)
        };
        let (details, details_view) = open(PanelKind::OrderDetails)?;
        let (flow, flow_view) = open(PanelKind::TradeDetails)?;
        let (book, book_view) = open(PanelKind::OrderBook)?;
        for handle in &handles {
            cx.update_window(*handle, |_, window, cx| window.render_frame(cx))?;
        }
        cx.update_window(details, |_, window, cx| {
            ensure!(
                window.find("market-min-amount").visible(),
                "Market amount filter is not visible"
            );
            window.click("market-min-amount", cx);
            window.input("5", cx);
            window.click("market-max-amount", cx);
            window.input("20", cx);
            window.click("market-min-quantity", cx);
            window.input("100", cx);
            window.click("market-direction-1", cx);
            window.click("market-filter-enabled", cx);
            let snapshot = details_view.read(cx).snapshot(cx);
            ensure!(
                snapshot["order_filter"]["enabled"] == true,
                "Market filter did not toggle"
            );
            ensure!(
                snapshot["order_filter"]["min_amount"] == "5",
                "Market amount input not saved"
            );
            ensure!(
                snapshot["order_filter"]["max_amount"] == "20",
                "Market amount upper bound not saved"
            );
            ensure!(
                snapshot["order_filter"]["min_quantity"] == "100",
                "Market quantity input not saved"
            );
            ensure!(
                snapshot["order_filter"]["direction"] == 1,
                "Market direction did not change"
            );
            // Rebuilding the content, as a cross-window move does, must keep the filters.
            let restored = cx.new(|cx| {
                PanelContent::new(
                    PanelKind::OrderDetails,
                    Symbol::Aapl,
                    Period::Intraday,
                    window,
                    cx,
                )
            });
            restored.update(cx, |view, cx| view.restore(&snapshot, window, cx))?;
            ensure!(
                restored.read(cx).snapshot(cx) == snapshot,
                "Market filter snapshot failed to restore"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        cx.update_window(flow, |_, window, cx| {
            window.click("flow-large", cx);
            ensure!(
                flow_view.read(cx).snapshot(cx)["large_trades_only"] == true,
                "Large execution filter did not change"
            );
            window.click("flow-all", cx);
            ensure!(
                flow_view.read(cx).snapshot(cx)["large_trades_only"] == false,
                "Execution filter did not reset"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        cx.update_window(book, |_, window, cx| {
            window.click("book-queue", cx);
            ensure!(
                book_view.read(cx).snapshot(cx)["book_queue"] == true,
                "Book queue toggle did not change"
            );
            window.click("book-depth", cx);
            ensure!(
                book_view.read(cx).snapshot(cx)["book_queue"] == false,
                "Book depth toggle did not reset"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        println!(
            "PASS: market order amount/quantity/direction filters, restored filter state, large execution filter and order-book queue controls"
        );
        Ok(())
    })();
    for handle in handles {
        let _ = cx.update_window(handle, |_, window, _| window.remove_window());
    }
    result
}

/// Regression for resource isolation and transactional content restoration.
/// Runs without a screenshot renderer, so Windows can execute it as well.
#[cfg(feature = "test-support")]
pub(crate) fn smoke_panel_state(cx: &mut HeadlessAppContext) -> anyhow::Result<()> {
    crate::panels::chart::smoke_data_source(cx)?;
    use anyhow::ensure;
    let mut handles = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        for kind in PanelKind::ALL {
            let open = |cx: &mut App| {
                gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                    cx.new(|cx| PanelContent::new(kind, Symbol::Aapl, Period::Day, window, cx))
                })
            };
            let (source, view) = cx.update(open)?;
            handles.push(source);
            let expected_inputs = match kind {
                PanelKind::Watchlist | PanelKind::OrderEntry => 2,
                PanelKind::OrderDetails => 4,
                _ => 0,
            };
            let (snapshot, source_ids) = cx.update_window(source, |_, window, cx| {
                let panel = view.read(cx);
                ensure!(
                    panel.view.inputs().len() == expected_inputs,
                    "Unexpected native inputs for {kind:?}"
                );
                ensure!(
                    matches!(panel.view, PanelView::Chart(_)) == (kind == PanelKind::Chart),
                    "Hidden chart allocated for {kind:?}"
                );
                ensure!(
                    matches!(panel.view, PanelView::Watchlist { .. })
                        == (kind == PanelKind::Watchlist),
                    "Hidden list editor allocated for {kind:?}"
                );
                let source_ids: Vec<_> = panel
                    .view
                    .inputs()
                    .iter()
                    .map(|input| input.entity_id())
                    .collect();
                let mut legacy = panel.snapshot(cx);
                legacy["query"] = serde_json::json!("legacy-query");
                legacy["price"] = serde_json::json!("007.50");
                legacy["quantity"] = serde_json::json!("50");
                legacy["side"] = serde_json::json!("Sell");
                legacy["sort"] = serde_json::json!(2);
                legacy["chart"] = serde_json::json!({"bars_visible": 80, "offset_from_latest": 3});
                legacy["order_filter"]["min_amount"] = serde_json::json!("unfinished");
                legacy["order_filter"]["direction"] = serde_json::json!(1);
                view.update(cx, |panel, cx| panel.restore(&legacy, window, cx))?;
                let before = view.read(cx).snapshot(cx);
                ensure!(
                    before == legacy,
                    "Unused legacy fields were lost by {kind:?}"
                );
                let mut corrupted = before.clone();
                corrupted["query"] = serde_json::json!(42);
                ensure!(
                    view.update(cx, |panel, cx| panel.restore(&corrupted, window, cx))
                        .is_err()
                );
                ensure!(
                    view.read(cx).snapshot(cx) == before,
                    "Failed restore mutated live {kind:?}"
                );
                view.update(cx, |panel, cx| {
                    panel.restore(&serde_json::Value::Null, window, cx)
                })?;
                ensure!(
                    view.read(cx).snapshot(cx) == before,
                    "Absent legacy content changed {kind:?}"
                );
                Ok::<_, anyhow::Error>((before, source_ids))
            })??;

            // Reconstruction uses a different native window and must never
            // reuse input entities or subscriptions from the source window.
            let (destination, rebuilt) = cx.update(open)?;
            handles.push(destination);
            cx.update_window(destination, |_, window, cx| {
                rebuilt.update(cx, |panel, cx| panel.restore(&snapshot, window, cx))?;
                ensure!(
                    rebuilt.read(cx).snapshot(cx) == snapshot,
                    "Migration changed {kind:?} content"
                );
                ensure!(
                    rebuilt
                        .read(cx)
                        .view
                        .inputs()
                        .iter()
                        .all(|input| !source_ids.contains(&input.entity_id())),
                    "Migration reused a source input entity"
                );
                Ok::<_, anyhow::Error>(())
            })??;
        }
        println!(
            "PASS: per-kind native resources, invalid content rollback, absent legacy content and cross-window snapshot reconstruction"
        );
        Ok(())
    })();
    for handle in handles {
        let _ = cx.update_window(handle, |_, window, _| window.remove_window());
    }
    result
}
