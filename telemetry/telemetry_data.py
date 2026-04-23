from __future__ import annotations

import csv
import io
from dataclasses import dataclass
from pathlib import Path
from typing import Any


REQUIRED_COLUMNS = ("sm_id", "warp_id", "thread_id")
OPTIONAL_COORDINATE_COLUMNS = (
    "grid_x",
    "grid_y",
    "grid_z",
    "thread_x",
    "thread_y",
    "thread_z",
)
OPTIONAL_STATUS_COLUMNS = ("warp_state",)
KNOWN_COLUMNS = set(REQUIRED_COLUMNS) | set(OPTIONAL_COORDINATE_COLUMNS) | set(OPTIONAL_STATUS_COLUMNS)


class TelemetryDataError(ValueError):
    """Raised when CSV telemetry data is missing required structure."""


@dataclass(frozen=True)
class ThreadRecord:
    sm_id: Any
    warp_id: Any
    thread_id: Any
    grid_x: Any | None
    grid_y: Any | None
    grid_z: Any | None
    thread_x: Any | None
    thread_y: Any | None
    thread_z: Any | None
    warp_state: Any | None
    extra_attributes: dict[str, Any]
    raw_attributes: dict[str, Any]


@dataclass(frozen=True)
class WarpSummary:
    sm_id: Any
    warp_id: Any
    thread_count: int
    warp_state: Any | None
    shared_attributes: dict[str, Any]


@dataclass(frozen=True)
class SMSummary:
    sm_id: Any
    warp_count: int
    thread_count: int
    warp_states: tuple[Any, ...]
    shared_attributes: dict[str, Any]


@dataclass(frozen=True)
class TelemetryDataset:
    source_name: str
    columns: tuple[str, ...]
    extra_columns: tuple[str, ...]
    threads: tuple[ThreadRecord, ...]
    sm_summaries: tuple[SMSummary, ...]
    warp_summaries: tuple[WarpSummary, ...]

    @property
    def total_sms(self) -> int:
        return len(self.sm_summaries)

    @property
    def total_warps(self) -> int:
        return len(self.warp_summaries)

    @property
    def total_threads(self) -> int:
        return len(self.threads)

    @property
    def threads_per_warp_values(self) -> tuple[int, ...]:
        return tuple(sorted({warp.thread_count for warp in self.warp_summaries}))

    @property
    def distinct_warp_states(self) -> tuple[Any, ...]:
        return tuple(sorted({warp.warp_state for warp in self.warp_summaries if warp.warp_state is not None}))

    def warps_for_sm(self, sm_id: Any) -> list[WarpSummary]:
        return [warp for warp in self.warp_summaries if warp.sm_id == sm_id]

    def threads_for_warp(self, sm_id: Any, warp_id: Any) -> list[ThreadRecord]:
        return [thread for thread in self.threads if thread.sm_id == sm_id and thread.warp_id == warp_id]

    def sm_by_id(self, sm_id: Any) -> SMSummary:
        for sm in self.sm_summaries:
            if sm.sm_id == sm_id:
                return sm
        raise KeyError(f"Unknown SM id: {sm_id}")

    def warp_by_ids(self, sm_id: Any, warp_id: Any) -> WarpSummary:
        for warp in self.warp_summaries:
            if warp.sm_id == sm_id and warp.warp_id == warp_id:
                return warp
        raise KeyError(f"Unknown warp ids: sm={sm_id}, warp={warp_id}")


def load_telemetry_csv(path: str | Path | None = None, content: bytes | None = None, source_name: str | None = None) -> TelemetryDataset:
    if path is None and content is None:
        raise TelemetryDataError("Provide either a CSV path or uploaded content.")

    if content is not None:
        text = content.decode("utf-8")
        effective_source = source_name or "uploaded.csv"
    else:
        csv_path = Path(path).expanduser()
        if not csv_path.exists():
            raise TelemetryDataError(f"CSV file not found: {csv_path}")
        text = csv_path.read_text(encoding="utf-8")
        effective_source = source_name or csv_path.name

    reader = csv.DictReader(io.StringIO(text))
    if reader.fieldnames is None:
        raise TelemetryDataError("CSV is empty or missing a header row.")

    columns = tuple(reader.fieldnames)
    missing_columns = [column for column in REQUIRED_COLUMNS if column not in columns]
    if missing_columns:
        raise TelemetryDataError(
            "CSV is missing required columns: " + ", ".join(missing_columns)
        )

    threads: list[ThreadRecord] = []
    for row in reader:
        normalized_row = {key: _coerce_value(value) for key, value in row.items()}
        thread = ThreadRecord(
            sm_id=normalized_row["sm_id"],
            warp_id=normalized_row["warp_id"],
            thread_id=normalized_row["thread_id"],
            grid_x=normalized_row.get("grid_x"),
            grid_y=normalized_row.get("grid_y"),
            grid_z=normalized_row.get("grid_z"),
            thread_x=normalized_row.get("thread_x"),
            thread_y=normalized_row.get("thread_y"),
            thread_z=normalized_row.get("thread_z"),
            warp_state=normalized_row.get("warp_state"),
            extra_attributes={key: value for key, value in normalized_row.items() if key not in KNOWN_COLUMNS},
            raw_attributes=normalized_row,
        )
        threads.append(thread)

    if not threads:
        raise TelemetryDataError("CSV does not contain any telemetry rows.")

    sm_summaries = _build_sm_summaries(threads)
    warp_summaries = _build_warp_summaries(threads)
    extra_columns = tuple(column for column in columns if column not in KNOWN_COLUMNS)
    return TelemetryDataset(
        source_name=effective_source,
        columns=columns,
        extra_columns=extra_columns,
        threads=tuple(sorted(threads, key=lambda thread: (thread.sm_id, thread.warp_id, thread.thread_id))),
        sm_summaries=tuple(sm_summaries),
        warp_summaries=tuple(warp_summaries),
    )


def _build_sm_summaries(threads: list[ThreadRecord]) -> list[SMSummary]:
    grouped: dict[Any, list[ThreadRecord]] = {}
    for thread in threads:
        grouped.setdefault(thread.sm_id, []).append(thread)

    summaries: list[SMSummary] = []
    for sm_id in sorted(grouped):
        sm_threads = grouped[sm_id]
        warp_ids = {thread.warp_id for thread in sm_threads}
        warp_states = tuple(sorted({thread.warp_state for thread in sm_threads if thread.warp_state is not None}))
        summaries.append(
            SMSummary(
                sm_id=sm_id,
                warp_count=len(warp_ids),
                thread_count=len(sm_threads),
                warp_states=warp_states,
                shared_attributes=_shared_attributes([thread.raw_attributes for thread in sm_threads], excluded_keys={"sm_id", "warp_id", "thread_id"}),
            )
        )
    return summaries


def _build_warp_summaries(threads: list[ThreadRecord]) -> list[WarpSummary]:
    grouped: dict[tuple[Any, Any], list[ThreadRecord]] = {}
    for thread in threads:
        grouped.setdefault((thread.sm_id, thread.warp_id), []).append(thread)

    summaries: list[WarpSummary] = []
    for sm_id, warp_id in sorted(grouped):
        warp_threads = grouped[(sm_id, warp_id)]
        warp_state = _single_value([thread.warp_state for thread in warp_threads if thread.warp_state is not None])
        summaries.append(
            WarpSummary(
                sm_id=sm_id,
                warp_id=warp_id,
                thread_count=len(warp_threads),
                warp_state=warp_state,
                shared_attributes=_shared_attributes(
                    [thread.raw_attributes for thread in warp_threads],
                    excluded_keys={"sm_id", "warp_id", "thread_id", "thread_x", "thread_y", "thread_z"},
                ),
            )
        )
    return summaries


def _shared_attributes(rows: list[dict[str, Any]], excluded_keys: set[str]) -> dict[str, Any]:
    if not rows:
        return {}

    shared: dict[str, Any] = {}
    for key in rows[0]:
        if key in excluded_keys:
            continue
        value = rows[0][key]
        if all(row.get(key) == value for row in rows[1:]):
            shared[key] = value
    return shared


def _single_value(values: list[Any]) -> Any | None:
    distinct = {value for value in values}
    if not distinct:
        return None
    if len(distinct) == 1:
        return next(iter(distinct))
    return "mixed"


def _coerce_value(value: str | None) -> Any:
    if value is None:
        return None
    stripped = value.strip()
    if stripped == "":
        return None
    for cast in (int, float):
        try:
            return cast(stripped)
        except ValueError:
            continue
    return stripped
