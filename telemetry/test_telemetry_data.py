from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from telemetry_data import TelemetryDataError, load_telemetry_csv


class TelemetryDataTests(unittest.TestCase):
    def test_loads_sample_csv_and_aggregates_counts(self) -> None:
        dataset = load_telemetry_csv(path="test.csv")

        self.assertEqual(dataset.total_sms, 5)
        self.assertEqual(dataset.total_warps, 50)
        self.assertEqual(dataset.total_threads, 1600)
        self.assertEqual(dataset.threads_per_warp_values, (32,))
        self.assertEqual(dataset.distinct_warp_states, (0, 2))

        warp_zero = dataset.warp_by_ids(0, 0)
        self.assertEqual(warp_zero.thread_count, 32)

        thread_zero = dataset.threads_for_warp(0, 0)[0]
        self.assertEqual((thread_zero.grid_x, thread_zero.grid_y, thread_zero.grid_z), (0, 0, 0))
        self.assertEqual((thread_zero.thread_x, thread_zero.thread_y, thread_zero.thread_z), (0, 0, 0))

    def test_surfaces_extra_columns_as_dynamic_metadata(self) -> None:
        csv_text = "\n".join(
            [
                "sm_id,warp_id,thread_id,grid_x,cuda_core_id,scheduler_id",
                "0,0,0,1,cc0,sched0",
                "0,0,1,1,cc1,sched0",
            ]
        )

        with tempfile.NamedTemporaryFile("w", delete=False, suffix=".csv") as handle:
            handle.write(csv_text)
            temp_path = Path(handle.name)

        try:
            dataset = load_telemetry_csv(path=temp_path)
        finally:
            temp_path.unlink(missing_ok=True)

        self.assertEqual(dataset.extra_columns, ("cuda_core_id", "scheduler_id"))
        thread = dataset.threads_for_warp(0, 0)[1]
        self.assertEqual(thread.extra_attributes["cuda_core_id"], "cc1")
        self.assertEqual(thread.extra_attributes["scheduler_id"], "sched0")

    def test_missing_required_columns_raise_clear_error(self) -> None:
        csv_text = "\n".join(
            [
                "sm_id,thread_id,grid_x",
                "0,0,1",
            ]
        )

        with self.assertRaises(TelemetryDataError) as context:
            load_telemetry_csv(content=csv_text.encode("utf-8"), source_name="broken.csv")

        self.assertIn("warp_id", str(context.exception))


if __name__ == "__main__":
    unittest.main()
