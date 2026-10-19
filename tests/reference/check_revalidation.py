#!/usr/bin/env python3
"""Verify a repeated pinned-R capture against unchanged strict check records.

Usage: python3 tests/reference/check_revalidation.py comparison.json
No fitting, tolerance adjustment, or discrepancy reclassification occurs here.
"""
import hashlib
import json
from pathlib import Path
import sys


def signature(checks):
    return hashlib.sha256(json.dumps(checks, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def verify(report, summary):
    assert report['reference_commit'] == summary['reference_commit']
    assert report['source_sha256'] == summary['reference_source_sha256']
    for key, value in summary['reference_versions'].items():
        assert report['reference_environment'][key] == value, f'Reference {key} changed'
    assert report['status_counts'] == summary['status_counts'], 'Strict classifications changed'
    actual = {c['name']: c for c in report['cases']}
    assert set(actual) == set(summary['cases']), 'Case set changed'
    for name, expected in summary['cases'].items():
        case = actual[name]
        assert case['reference_completed'] and case['reference_returncode'] == 0, f'{name}: R incomplete'
        for group, digest in expected.items():
            assert signature(case[group]) == digest, f'{name}/{group}: numerical checks or classifications changed'
    return len(actual)


if __name__ == '__main__':
    summary = json.loads(Path(__file__).with_name('revalidation-summary.json').read_text())
    count = verify(json.loads(Path(sys.argv[1]).read_text()), summary)
    print(f'{count} completed R cases reproduce every strict check; original GAM failures remain failures.')
