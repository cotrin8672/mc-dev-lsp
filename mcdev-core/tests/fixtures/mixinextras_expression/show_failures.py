from pathlib import Path
import json
def diff(a,b,path=''):
    if type(a)!=type(b):return [(path,a,b)]
    if isinstance(a,dict):return sum((diff(a.get(k),b.get(k),path+'.'+k)for k in sorted(a.keys()|b.keys())),[])
    if isinstance(a,list):
        if len(a)!=len(b):return [(path+'.length',len(a),len(b))]
        return sum((diff(x,y,path+'['+str(i)+']')for i,(x,y)in enumerate(zip(a,b))),[])
    return [] if a==b else [(path,a,b)]
root=Path(__file__).resolve().parents[4]
for row in json.loads((root/'build/rust-test-logs/expression-source-failures.json').read_text(encoding='utf8')):
    print(row['line'],row['case'].split('::')[-1],diff(row.get('actual'),row.get('expected'))[:4])
