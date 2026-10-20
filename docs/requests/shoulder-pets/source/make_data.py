import base64
from pathlib import Path
here=Path(__file__).parent
items={name:base64.b64encode((here/(f'{name}.glb' if name=='ember_hatchling' else
                                    f'{name}-textured.glb')).read_bytes()).decode('ascii')
       for name in ('shadow_pup','ember_hatchling','cloud_wisp')}
import json
(here.parent/'data.js').write_text('window.MODEL_DATA='+json.dumps(items,separators=(',',':'))+';',encoding='utf-8')
