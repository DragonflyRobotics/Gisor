from __future__ import annotations

from typing import Any

import pandas as pd
import streamlit as st

from telemetry_data import TelemetryDataError, TelemetryDataset, ThreadRecord, load_telemetry_csv


DEFAULT_CSV_PATH = "test.csv"
THREAD_TABLE_COLUMNS = [
    "thread_id",
    "thread_x",
    "thread_y",
    "thread_z",
    "grid_x",
    "grid_y",
    "grid_z",
    "warp_state",
]


st.set_page_config(page_title="GPU Telemetry Explorer", layout="wide")


def main() -> None:
    st.title("GPU Telemetry Explorer")
    st.caption("Inspect SMs, warps, threads, and any future telemetry columns from your emulator CSV.")

    dataset = render_data_source_controls()
    if dataset is None:
        return

    render_summary(dataset)
    render_explorer(dataset)


def render_data_source_controls() -> TelemetryDataset | None:
    st.subheader("Data Source")
    source_col, upload_col = st.columns([2, 1])
    with source_col:
        csv_path = st.text_input("CSV path", value=DEFAULT_CSV_PATH, help="Used when no uploaded file is selected.")
    with upload_col:
        uploaded_file = st.file_uploader("Upload CSV", type=["csv"])

    try:
        if uploaded_file is not None:
            dataset = load_telemetry_csv(content=uploaded_file.getvalue(), source_name=uploaded_file.name)
        else:
            dataset = load_telemetry_csv(path=csv_path)
    except TelemetryDataError as exc:
        st.error(str(exc))
        return None

    st.caption(f"Loaded `{dataset.source_name}` with {len(dataset.columns)} columns.")
    if dataset.extra_columns:
        st.caption("Dynamic metadata columns: " + ", ".join(dataset.extra_columns))
    else:
        st.caption("No extra metadata columns detected yet.")
    return dataset


def render_summary(dataset: TelemetryDataset) -> None:
    st.subheader("Overview")
    metrics = st.columns(4)
    metrics[0].metric("SMs", dataset.total_sms)
    metrics[1].metric("Warps", dataset.total_warps)
    metrics[2].metric("Threads", dataset.total_threads)
    threads_per_warp = ", ".join(str(value) for value in dataset.threads_per_warp_values) or "n/a"
    metrics[3].metric("Threads / Warp", threads_per_warp)

    warp_states = ", ".join(str(value) for value in dataset.distinct_warp_states) or "None"
    st.write(f"Distinct warp states: `{warp_states}`")


def render_explorer(dataset: TelemetryDataset) -> None:
    st.subheader("Hierarchy Explorer")
    sm_col, warp_col, thread_col = st.columns([1.1, 1.2, 2.2], gap="large")

    sm_options = dataset.sm_summaries
    selected_sm = sm_col.radio(
        "Streaming Multiprocessors",
        sm_options,
        format_func=lambda sm: f"SM {sm.sm_id} ({sm.warp_count} warps, {sm.thread_count} threads)",
    )
    render_sm_details(sm_col, selected_sm)

    warp_options = dataset.warps_for_sm(selected_sm.sm_id)
    selected_warp = warp_col.radio(
        "Warps",
        warp_options,
        format_func=lambda warp: f"Warp {warp.warp_id} ({warp.thread_count} threads, state={warp.warp_state})",
    )
    render_warp_details(warp_col, selected_warp)

    render_thread_panel(thread_col, dataset, selected_warp.sm_id, selected_warp.warp_id)


def render_sm_details(container: Any, sm_summary: Any) -> None:
    with container:
        st.markdown("**Selected SM**")
        st.json(
            {
                "sm_id": sm_summary.sm_id,
                "warp_count": sm_summary.warp_count,
                "thread_count": sm_summary.thread_count,
                "warp_states": list(sm_summary.warp_states),
                "shared_attributes": sm_summary.shared_attributes,
            },
            expanded=False,
        )


def render_warp_details(container: Any, warp_summary: Any) -> None:
    with container:
        st.markdown("**Selected Warp**")
        st.json(
            {
                "sm_id": warp_summary.sm_id,
                "warp_id": warp_summary.warp_id,
                "thread_count": warp_summary.thread_count,
                "warp_state": warp_summary.warp_state,
                "shared_attributes": warp_summary.shared_attributes,
            },
            expanded=False,
        )


def render_thread_panel(container: Any, dataset: TelemetryDataset, sm_id: Any, warp_id: Any) -> None:
    threads = dataset.threads_for_warp(sm_id, warp_id)
    table_df = build_thread_table(threads, dataset.extra_columns)

    with container:
        st.markdown("**Threads**")
        selection = st.dataframe(
            table_df,
            use_container_width=True,
            hide_index=True,
            on_select="rerun",
            selection_mode="single-row",
        )

        selected_thread = resolve_selected_thread(threads, selection)
        selected_thread_id = selected_thread.thread_id if selected_thread is not None else threads[0].thread_id
        selected_thread = next(thread for thread in threads if thread.thread_id == selected_thread_id)

        options = [thread.thread_id for thread in threads]
        selected_thread_id = st.selectbox("Thread detail", options, index=options.index(selected_thread.thread_id))
        selected_thread = next(thread for thread in threads if thread.thread_id == selected_thread_id)

        st.markdown("**Selected Thread**")
        st.json(build_thread_detail(selected_thread), expanded=True)


def build_thread_table(threads: list[ThreadRecord], extra_columns: tuple[str, ...]) -> pd.DataFrame:
    rows: list[dict[str, Any]] = []
    for thread in threads:
        row = {}
        for column in THREAD_TABLE_COLUMNS:
            row[column] = thread.raw_attributes.get(column)
        for extra_column in extra_columns:
            row[extra_column] = thread.extra_attributes.get(extra_column)
        rows.append(row)
    ordered_columns = [column for column in THREAD_TABLE_COLUMNS if column in rows[0]]
    ordered_columns.extend(extra_columns)
    return pd.DataFrame(rows)[ordered_columns]


def resolve_selected_thread(threads: list[ThreadRecord], selection_event: Any) -> ThreadRecord | None:
    if not hasattr(selection_event, "selection"):
        return None
    rows = selection_event.selection.get("rows", [])
    if not rows:
        return None
    selected_index = rows[0]
    if selected_index >= len(threads):
        return None
    return threads[selected_index]


def build_thread_detail(thread: ThreadRecord) -> dict[str, Any]:
    detail = {
        "sm_id": thread.sm_id,
        "warp_id": thread.warp_id,
        "thread_id": thread.thread_id,
        "coordinates": {
            "grid": [thread.grid_x, thread.grid_y, thread.grid_z],
            "thread": [thread.thread_x, thread.thread_y, thread.thread_z],
        },
        "warp_state": thread.warp_state,
        "extra_attributes": thread.extra_attributes,
    }
    return detail


if __name__ == "__main__":
    main()
