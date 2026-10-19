"""Local documentation navigation checks; no Markdown package or network required."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_docs import anchors, check


class DocumentationTests(unittest.TestCase):
    def test_headings_and_compatibility_aliases_ignore_fenced_examples(self):
        text = '''<a id="previous-heading"></a>
## All / Q-score (#5)
## Repeated
## Repeated
```md
## Not a heading
[not a link](missing.md)
```
'''
        self.assertEqual(anchors(text), {
            'previous-heading', 'all--q-score-5', 'repeated', 'repeated-1'})

    def test_local_navigation_checks_paths_and_fragments_without_external_requests(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'guide.md').write_text('# Run grouping\n<a id="old-name"></a>\n')
            index = root / 'README.md'
            index.write_text('''[Guide](guide.md#run-grouping)
[Old link](guide.md#old-name)
[External](https://invalid.example/never-requested)
~~~sh
[Example](missing.md)
~~~
''')
            self.assertEqual(check([index]), (2, []))
            index.write_text('[Missing page](missing.md)\n[Missing section](guide.md#unknown)\n')
            total, failures = check([index])
            self.assertEqual(total, 2)
            self.assertEqual(len(failures), 2)
            self.assertIn('missing target missing.md', failures[0])
            self.assertIn('missing anchor guide.md#unknown', failures[1])


if __name__ == '__main__':
    unittest.main()
