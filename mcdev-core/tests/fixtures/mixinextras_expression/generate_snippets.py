"""Copy the baseline's static snippet data once; production consumes only JSON."""
from pathlib import Path
import importlib.util,re,json
ROOT=Path(__file__).resolve().parents[4]
spec=importlib.util.spec_from_file_location('helper',ROOT/'mcdev-core/tests/fixtures/mixin_source/generate_capture.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
source=(h.MAIN/'mixinextras/ExpressionSupport.kt').read_text(encoding='utf8').split('    fun completeAtValue')[0]
def literal(text):return json.loads(re.sub(r"\$\{\s*'\$'\s*\}",'$',text.strip()))
rows=[]
for m in re.finditer(r'FeatureSnippet\(',source):
 op=m.end()-1;end=h.closing(source,op);args=[a.strip()for a in h.split_params(source[op+1:end])if a.strip()]
 row=dict(trigger=literal(args[0]),label=literal(args[1]),insertText=literal(args[2]),detail='MixinExtras feature',source='mixinextras.feature')
 imports=[]
 for arg in args[3:]:
  if arg.startswith('source ='):row['source']=literal(arg.split('=',1)[1])
  elif arg.startswith('detail ='):row['detail']=literal(arg.split('=',1)[1])
  else:
   imports += re.findall(r'"([^"\\]+)"',arg)
   imports += ['@Mixin:'+name for name in re.findall(r'MixinAnnotation\.(\w+)\.officialFqns',arg)]
 row['imports']=imports;rows.append(row)
target=ROOT/'mcdev-core/src/mixinextras_expression/feature_snippets.json';target.write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf8');print('copied snippets:',len(rows))
