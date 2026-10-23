"""Guard evidence handling, not fitting a second statistical model."""
import copy
import unittest

from assess_stability import common_penalties, independent, inverse, maximum, multiply


class StabilityEvidenceTests(unittest.TestCase):
    def test_aliases_are_not_independent_backends(self):
        original = {'symbols': {s: {'path': '/a/lib.dylib', 'sha256': 'same'}
                               for s in ('dgemm_', 'dsyevr_')}}
        alias = copy.deepcopy(original)
        for record in alias['symbols'].values():
            record['path'] = '/different/alias.dylib'
        self.assertFalse(independent(original, alias))
        alias['symbols']['dgemm_']['sha256'] = 'other'
        self.assertFalse(independent(original, alias))
        alias['symbols']['dsyevr_']['sha256'] = 'other'
        self.assertTrue(independent(original, alias))

    def test_mismatched_or_nonfinite_coordinates_fail_closed(self):
        for a,b in [([1], [1,2]), ([float('nan')], [1]), ([1], [float('inf')])]:
            with self.assertRaises(AssertionError):
                maximum(a,b)

    def test_nodal_comparison_preserves_intercept_and_active_column_order(self):
        # The first Householder column is discarded, not the last. This guards
        # the centered/nodal conversion rather than comparing arbitrary bases.
        q = [[float(i==j) for j in range(10)] for i in range(10)]
        transform = [[1.0]+row[1:] for row in q]
        inv = inverse(transform)
        self.assertLess(maximum(multiply(transform,inv),q), 1e-14)
        centered = [[float(i==j)*(i+1) if i<9 else 0 for j in range(10)]
                    for i in range(10)]
        penalty = [[0.0]*10]+[[0.0]+row[:9] for row in centered[:9]]
        transpose = lambda x: list(map(list,zip(*x)))
        nodal = multiply(transpose(inv), multiply(penalty,inv))
        model = dict(rust_audit=dict(fit=dict(transform=q,centered_penalty=centered)),
            variants=dict(canonical=dict(penalty_scale=2, audit=dict(
                raw_eigenvalues=[4], effective_nodal_penalty=[[2*v for v in row] for row in nodal]))))
        self.assertEqual(common_penalties(model),0)
        model['variants']['canonical']['audit']['effective_nodal_penalty'][0][0] += 1
        self.assertEqual(common_penalties(model),.5)


if __name__ == '__main__':
    unittest.main()
