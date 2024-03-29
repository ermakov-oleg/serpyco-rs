import collections
import io
import json
import os

from tabulate import tabulate


LIBRARIES = (
    'serpyco_rs',
    'serpyco',
    'mashumaro',
    'pydantic',
    'marshmallow',
)


def aggregate():
    benchmarks_dir = os.path.join('.benchmarks', os.listdir('.benchmarks')[0])
    res = collections.defaultdict(dict)
    for filename in os.listdir(benchmarks_dir):
        with open(os.path.join(benchmarks_dir, filename)) as fileh:
            data = json.loads(fileh.read())

        for each in data['benchmarks']:
            res[each['group']][each['extra_info']['lib']] = {
                'data': [val * 1000 for val in each['stats']['data']],
                'median': each['stats']['median'] * 1000,
                'ops': each['stats']['ops'],
                'correct': each['extra_info']['correct'],
            }
    return res


def tab(obj):
    buf = io.StringIO()
    headers = (
        'Library',
        'Median latency (milliseconds)',
        'Operations per second',
        'Relative (latency)',
    )
    for group, val in sorted(obj.items(), reverse=True):
        buf.write('\n' + '#### ' + group + '\n\n')
        table = []
        for lib in LIBRARIES:
            correct = val[lib]['correct']
            table.append(
                [
                    lib,
                    val[lib]['median'] if correct else None,
                    '%.1f' % val[lib]['ops'] if correct else None,
                    0,
                ]
            )
        baseline = table[0][1]
        for each in table:
            each[3] = '%.2f' % (each[1] / baseline) if isinstance(each[1], float) else None
            each[1] = '%.2f' % each[1] if isinstance(each[1], float) else None
        buf.write(tabulate(table, headers, tablefmt='github') + '\n')

    print(buf.getvalue())


tab(aggregate())
