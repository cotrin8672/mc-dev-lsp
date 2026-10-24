"""Explicit fixture-only conversion of obsolete JDT signature/fake storage.
No production signature parser, callbacks or expected public result feed the facts.
Resolution candidates come from actual frozen resolveType callback observations.
"""
from pathlib import Path
import json,re
HERE=Path(__file__).resolve().parent
def primitive(name):return {'kind':'primitive','value':name}
def unknown():return {'kind':'unresolved'}
def class_type(name):return {'kind':'class','value':{'internal_name':name}}
def signature(text,resolutions):
    if not isinstance(text,str)or not text:return unknown()
    dimensions=len(text)-len(text.lstrip('['));body=text[dimensions:]
    primitives=dict(zip('VZBCSIJFD',['void','boolean','byte','char','short','int','long','float','double']))
    if body in primitives:result=primitive(primitives[body])
    elif body.startswith(('L','Q'))and body.endswith(';'):
        name=body[1:-1].split('<',1)[0]
        if body[0]=='L':result=class_type(name.replace('.','/'))
        else:result=resolutions.get(name,unknown())
    elif body.startswith('T')and body.endswith(';'):result={'kind':'erasure','value':class_type('java/lang/Object')}
    else:result=unknown()
    return {'kind':'array','value':{'element':result,'dimensions':dimensions}}if dimensions else result

def normalize_class(value,resolutions):
    fqn=value.get('fqn','');package=value.get('packageName','')
    relative=(fqn[len(package)+1:]if package else fqn).replace('.','$')
    internal=(package.replace('.','/')+'/'if package else '')+relative
    methods=[]
    for method in value.get('methods',[]):
        available=set(method.get('_methods',[]))
        parameters=method.get('parameterTypes')
        if method.get('_class','').endswith('$BadParameterTypesMethod'):parameters=['I',123]
        methods.append({'name':method.get('elementName',''),
            'isConstructor':bool(method.get('constructor',False))if'isConstructor/0'in available else None,
            'accessFlags':method.get('flags',0)if'getFlags/0'in available else None,
            'parameterTypes':[signature(p,resolutions)for p in parameters]if parameters is not None else[unknown()],
            'returnType':signature(method.get('returnType'),resolutions),
        })
    fields=[]
    for field in value.get('fields',[]):
        available=set(field.get('_methods',[]))
        fields.append({'name':field.get('elementName',''),
            'accessFlags':field.get('flags',0)if'getFlags/0'in available else None,
            'fieldType':signature(field.get('typeSignature'),resolutions)})
    return {'internalName':internal,'simpleName':relative,'packageName':package,
        'methods':methods,'fields':fields,
        'methodsComplete':'getMethods/0'in value.get('_methods',[]),
        'fieldsComplete':'getFields/0'in value.get('_methods',[])}

def clean(v):
    if isinstance(v,dict):return {k:clean(x)for k,x in v.items()if not k.startswith('_')}
    if isinstance(v,list):return [clean(x)for x in v]
    return v

def main():
    rows=[json.loads(line)for line in (HERE/'project-calls.jsonl').read_text().splitlines()]
    resolutions={};pending_pairs={}
    for row in rows:
        case=row['legacyCase'];api=row['api']
        if api.endswith('.uniquePackageTypePair'):pending_pairs[case]=row['args']['resolved']
        if api.endswith('.resolveTypeToInternalName'):
            pairs=pending_pairs.pop(case,None)
            if pairs is None:fact=unknown()
            else:
                candidates=[class_type((p[0].replace('.','/')+'/'if p[0]else'')+p[1].replace('.','$'))for p in pairs if isinstance(p,list)and len(p)==2 and all(isinstance(x,str)for x in p)]
                fact={'kind':'candidates','value':candidates}
            resolutions.setdefault(case,{})[row['args']['erasureName']]=fact
    normalized=[];frames=[]
    for index,row in enumerate(rows):
        if row.get('frameDepth',1)!=1:frames.append(row);continue
        api=row['api'].rsplit('.',1)[-1]
        if api not in ['findClass','findClassByFqn','findClasses','getMethods','getFields']:
            frames=[];continue
        lookup=next((r for r in reversed(frames)if r['api']=='JdtProjectSourceQuery.findSourceType'),None)
        value=lookup.get('output')if lookup else None
        fact={'status':'known','value':normalize_class(value,resolutions.get(row['legacyCase'],{}))}if value else{'status':'unavailable','value':{'reason':'source type resolution unavailable'}}
        failure=row.get('throwable')
        error=None
        if failure:error={'kind':'cancelled'if failure.endswith('CancellationException')else'interrupted'if failure.endswith('InterruptedException')else'failure','message':row.get('message','')}
        expected=clean(row.get('output'))
        if api=='findClasses':
            stored=row.get('output')
            if '$thisAsSequence$inlined'in stored:expected=clean(stored['$thisAsSequence$inlined'])
            elif '$this_asSequence$inlined'in stored:expected=clean(stored['$this_asSequence$inlined'])
            elif stored.get('_class','').endswith('EmptySequence'):expected=[]
            else:raise ValueError(('unrecognized lazy sequence snapshot',stored))
        normalized.append({'legacyCase':row['legacyCase'],'captureRow1Based':index+1,'api':api,'args':clean(row['args']),'classFact':fact,'error':error,'expected':expected})
        frames=[]
    (HERE/'project-normalized.json').write_text(json.dumps(normalized,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    print(json.dumps({'publicCalls':len(normalized),'caseCount':len({r['legacyCase']for r in normalized}),'errorCalls':sum(r['error']is not None for r in normalized)}))
if __name__=='__main__':main()
