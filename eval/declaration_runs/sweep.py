import json, subprocess, sys, collections, os

repos = sorted(os.listdir('bench/repos'))
NOSE = './target/release/nose'
decl = []
for r in repos:
    try:
        out = subprocess.run([NOSE,'scan',f'bench/repos/{r}','--top','0','--format','json'],
                             capture_output=True, text=True, timeout=600)
        d = json.loads(out.stdout)
    except Exception as e:
        print(f'{r}: ERROR {e}', file=sys.stderr); continue
    for f in d.get('families', []):
        if f.get('recommended_surface') == 'declaration':
            decl.append({'repo': r, 'family_id': f['family_id'],
                         'locations': [(l['file'], l['start_line'], l['end_line']) for l in f['locations']]})
with open('/tmp/decl_families.json','w') as fh: json.dump(decl, fh, indent=1)

print(f'declaration families corpus-wide: {len(decl)} across {len(set(d["repo"] for d in decl))} repos')
bylang = collections.Counter(d['locations'][0][0].rsplit('.',1)[-1] for d in decl)
print('by extension:', dict(bylang.most_common()))

labels = json.load(open('bench/labels/refactoring_families.v5.json'))['families']
spans = collections.defaultdict(list)
for fam in labels:
    for m in fam['members']:
        spans[m['file']].append((m['start_line'], m['end_line'], fam.get('worthy'), fam['family_id']))

hits, worthy_hits = [], []
for d in decl:
    for file, s, e in d['locations']:
        for (ls, le, worthy, lfid) in spans.get(file, []):
            if s <= le and ls <= e:
                hits.append((d['repo'], d['family_id'], file, s, e, worthy, lfid))
                if worthy: worthy_hits.append(hits[-1])
print(f'label-span overlaps: {len(hits)} total; WORTHY overlaps: {len(worthy_hits)}')
for h in worthy_hits[:10]: print('  WORTHY:', h)
