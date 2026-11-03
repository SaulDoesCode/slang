(()=>{var N={"\u27EA":"fork","\xA6":"lane","\u27EB":"merge","\u2A1D":"join"},T={"[[":"fork","||":"lane","]]":"merge","||>":"join"},U={fork:"\u27EA",lane:"\xA6",merge:"\u27EB",join:"\u2A1D"},b=class extends Error{constructor(t,s){super(t),this.at=s}};function K(n){let t=0,s=n.length,e=[];for(;t<s;){let r=n[t];if(!r.m){e.push(r),t++;continue}if(r.m!=="fork")throw new b(`${U[r.m]} outside a fork`,r.at);e.push(i())}return e;function i(){let r=n[t],o=r.line,a=[],c=!1;for(;t<s;){let h=n[t];if(h.m==="fork"){let _=l();a.push(_),o=_.endLine;continue}if(h.m==="join"){t++,c=!0;break}if(h.m)throw new b(`${U[h.m]} outside a fork`,h.at);if(h.line!==o)break;a.push(h),t++}return{region:a,joined:c,at:r.at}}function l(){let r=n[t++],o=[[]],a=-1;for(;;){if(t>=s)throw new b("\u27EA without \u27EB",r.at);let c=n[t],h=o[o.length-1];if(c.m==="merge")return t++,{fork:o,label:r.label||"",at:r.at,endLine:c.line};if(c.m==="lane"){t++,o.push([]),a=-1;continue}if(c.m==="fork"){a<0&&(a=h.length),h.push(l());continue}if(c.m==="join"){if(a<0)throw new b("\u2A1D with no open fork",c.at);let _=h.splice(a);h.push({region:_,joined:!0,at:_[0].at}),a=-1,t++;continue}h.push(c),t++}}}function I(n){let t=new Map;for(let l of n){if(!l.fork)continue;let r=l.fork.length,o=t.get(l.label);if(o===void 0||o===1)t.set(l.label,r);else if(r!==1&&r!==o)throw new b(`this \u27EA has ${r} lanes where its set has ${o}`,l.at)}let s=[...t.keys()],e=[[]];for(let l of s){let r=[];for(let o of e)for(let a=0;a<t.get(l);a++)r.push([...o,a]);e=r}let i=[];for(let l of e){let r=[[]];for(let o of n){if(!o.fork){for(let _ of r)_.push(o);continue}let a=o.fork.length===1?o.fork[0]:o.fork[l[s.indexOf(o.label)]],c=I(a),h=[];for(let _ of r)for(let f of c)h.push(_.concat(f));r=h}i.push(...r)}return i}var nt="?!~$`#;:.@",it=["=","n","r","rc","rn","$","err","eof","out","threads","_","args","argc","?"],L=0,rt=1,lt=2,ot=3,X=4,at=5,G=6,Z=7;var ht=10,ct=11,_t=12,ft=13,Q=1/0;function C(n){if(n===0)return"0";if(Number.isNaN(n))return"NaN";if(n===1/0)return"inf";if(n===-1/0)return"-inf";let t=String(n);if(!t.includes("e"))return t;let s=t.match(/^(-?)(\d)(?:\.(\d+))?e([+-]\d+)$/),e=s[1],i=s[2]+(s[3]||""),l=+s[4];return l>=0?e+(l>=i.length-1?i+"0".repeat(l-(i.length-1)):i.slice(0,l+1)+"."+i.slice(l+1)):e+"0."+"0".repeat(-l-1)+i}function D(n){if(!/^[+-]?((\d+\.?\d*|\.\d+)([eE][+-]?\d+)?|inf|infinity|nan)$/i.test(n))return null;let t=n.toLowerCase().replace(/^\+/,"");return t==="inf"||t==="infinity"?1/0:t==="-inf"||t==="-infinity"?-1/0:t==="nan"||t==="-nan"?NaN:Number(n)}function ut(n){let t=D(n.trim());return t===null?0:t}function J(n){let t=D(n);return t===null?null:C(t)===n?t:null}var p=n=>{let t=J(n);return t===null?n:t},u=n=>typeof n=="number"?C(n):typeof n=="string"?n:n.w,g=n=>typeof n=="number"?n:ut(u(n));function j(n,t){return typeof n=="number"&&typeof t=="number"&&!Number.isNaN(n)&&!Number.isNaN(t)?n===t:u(n)===u(t)}var A=n=>n===" "||n===`
`||n==="	"||n==="\r",dt=n=>/[ \n\t\r]/.test(n);function pt(n){let t=Array.from(n);return t.length>1&&(t[0]==="\\"||nt.includes(t[t.length-1]))}var Y=[">",">_","<","<_","w","+w","mv","r","r@","?)","&&&","***","n","+","-","*","/","^","%","!!","=","!=",";","&","==","<<",">>","<<=",">>=","l","-l","[l]","l=","l-","#l","/l","l/","#s","f","+f","@f","f-","f_","_f","?eof","t","-t","<t",">t","#prng","`prng","><","//","<-","<--","(",")","~","##","tcp"],mt=new Set(Y),W=n=>mt.has(n)?n:null,tt=Y,z=class{constructor(){this.map=new Map,this.names=[],this.local=[];for(let t of it)this.sym(t)}sym(t){let s=this.map.get(t);return s!==void 0||(s=this.names.length,this.names.push(t),this.local.push(t.length>1&&t[0]===":"),this.map.set(t,s)),s}name(t){return this.names[t]}};function k(n,t){let s=W(n);if(s)return["cmd",s];let e=Array.from(n);if(e.length>1&&e[0]==="\\"){let r=e.slice(1).join(""),o=W(r);if(o)return["native",o];let a=k(r,t);return a[0]==="word"?["push",p(a[1])]:a}let i=e[e.length-1],l=e.slice(0,-1).join("");if(l)switch(i){case"?":return["cond",!0,k(l,t)];case"!":return["cond",!1,k(l,t)];case"~":return["lookup",t.sym(l)];case"@":return["pop",t.sym(l)];case"$":return["def",t.sym(l)];case"`":return["setmem",t.sym(l)];case"#":return["setnum",t.sym(l)];case";":return["inc",t.sym(l)];case":":return["dec",t.sym(l)];case".":{let r=Array.from(l);return r.length>1&&r[r.length-1]==="."?["callpass",t.sym(r.slice(0,-1).join(""))]:["call",t.sym(l)]}}return["word",n,t.sym(n),J(n)]}function st(n,t,s){if(!t)return["word",n,s.sym(n),null];if(t==="..")return["callpass",s.sym(n)];let e=Array.from(t),i=e[e.length-1],l=e.slice(0,-1).join("");if(i==="?"||i==="!")return["cond",i==="?",st(n,l,s)];if(l)throw new Error(`\u27E8${n}\u27E9${t}: unknown suffix`);let r={"~":"lookup","@":"pop",$:"def","`":"setmem","#":"setnum",";":"inc",":":"dec",".":"call"}[i];if(!r)throw new Error(`\u27E8${n}\u27E9${t}: unknown suffix`);return[r,s.sym(n)]}function gt(n,t,s){let e=n.length,i=1,l=t+1,r="",o=[];for(;l<e;){let a=n[l];if(a==="{")i++,r+=a,l++;else if(a==="}"){if(i--,i===0)return r&&o.push(["T",r]),[o,l+1];r+=a,l++}else if(a==="~"&&i===1){let c=l+1;for(;c<e&&n[c]!=="~"&&n[c]!=="{"&&n[c]!=="}"&&!A(n[c]);)c++;c<e&&n[c]==="~"?(c===l+1?r+="~":(r&&(o.push(["T",r]),r=""),o.push(["V",s.sym(n.slice(l+1,c).join(""))])),l=c+1):(r+="~",l++)}else r+=a,l++}throw new Error("unclosed {")}function bt(n,t){let s=[];for(let e of[["T",n],...t]){if(e[0]==="T"&&!e[1])continue;let i=s[s.length-1];e[0]==="T"&&i&&i[0]==="T"?i[1]+=e[1]:s.push([...e])}return s.every(e=>e[0]==="T")?["push",p(s.length?s[s.length-1][1]:"")]:["interp",s]}function H(n,t){let s=Array.from(n),e=s.length,i=[],l=[],r=0;for(;r<e;){if(A(s[r])){r++;continue}let o=r;if(s[r]==="\u27E8"){let _=r+1;for(;_<e&&s[_]!=="\u27E9";)_++;if(_>=e)throw new Error("unclosed \u27E8");let f=s.slice(r+1,_).join(""),d=_+1;for(;d<e&&!A(s[d]);)d++;i.push(st(f,s.slice(_+1,d).join(""),t)),l.push(o),r=d;continue}let a="",c=!1,h=o;for(;r<e&&!A(s[r]);){let _=s[r];if(N[_]){a&&(i.push(k(a,t)),l.push(h),a="");let f="";if(N[_]==="fork"){let d=/^([A-Za-z_][\w-]*):(?=[^\s⟪¦⟫⨝])/.exec(s.slice(r+1,r+40).join(""));d&&(f=d[1],r+=Array.from(d[0]).length)}i.push(["mark",N[_],f]),l.push(r),r++,h=r;continue}if(_==="{"){let[f,d]=gt(s,r,t);i.push(bt(a,f)),a="",l.push(h),r=d,c=!0;break}else if(_==="'"){let f=r+1;for(;f<e&&s[f]!=="'";)f++;if(f>=e)throw new Error("unclosed '");i.push(["push",p(a+s.slice(r+1,f).join(""))]),a="",l.push(h),r=f+1,c=!0;break}else a+=_,r++}if(!c&&a){let _=T[a]?[T[a],""]:/^\[\[([A-Za-z_][\w-]*):$/.test(a)?["fork",a.slice(2,-1)]:null;i.push(_?["mark",_[0],_[1]]:k(a,t)),l.push(h)}}return wt({ops:i,spans:l,src:n,unit:-1},s,t)}function wt(n,t,s){if(!n.ops.some(h=>h[0]==="mark"))return n;let e=[],i=0;for(let h=0;h<=t.length;h++)e.push(i),t[h]===`
`&&i++;let l=n.ops.map((h,_)=>h[0]==="mark"?{m:h[1],label:h[2],at:n.spans[_],line:e[n.spans[_]]}:{v:h,at:n.spans[_],line:e[n.spans[_]]}),r;try{r=K(l)}catch(h){throw h instanceof b?new Error(`at ${h.at}: ${h.message}`):h}let o=h=>{let _=[],f=[];for(let d of h)d.region?(_.push(a(d)),f.push(d.at)):(_.push(d.v),f.push(d.at));return{ops:_,spans:f,src:n.src,unit:n.unit}},a=h=>{let _=h.region.filter(m=>m.fork);if(_.length===1&&_[0].fork.length===1&&_[0].fork[0].length===1&&_[0].fork[0][0].v){let m=_[0].fork[0][0].v,q=m[0]==="word"&&m[1].endsWith("\u2026")&&m[1].length>1?m[1].slice(0,-1):m[0]==="callpass"&&s.name(m[1]).endsWith(".")?s.name(m[1]).slice(0,-1):null;if(q)return["lanes",{spread:s.sym(q),shared:o(h.region.filter($=>!$.fork)),lanes:[],at:h.at}]}let f;try{f=I(h.region)}catch(m){throw m instanceof b?new Error(`at ${m.at}: ${m.message}`):m}let d=_.every(m=>m.fork.every(q=>q.every($=>$.v&&$.v[0]==="push")));return["lanes",{lanes:f.map(o),simd:d,at:h.at}]},c=o(r);return c.unit=n.unit,c}var x=(1n<<64n)-1n,O=(n,t)=>(n<<BigInt(t)|n>>BigInt(64-t))&x;function R(n){n.z=n.z+0x9E3779B97F4A7C15n&x;let t=n.z;return t=(t^t>>30n)*0xBF58476D1CE4E5B9n&x,t=(t^t>>27n)*0x94D049BB133111EBn&x,t^t>>31n}var E=class{constructor(t,s){let e={z:(t^O(s,32))&x};this.s=[R(e),R(e),R(e),R(e)]}next(){let t=this.s,s=O(t[1]*5n&x,7)*9n&x,e=t[1]<<17n&x;return t[2]^=t[0],t[3]^=t[1],t[1]^=t[2],t[0]^=t[3],t[2]^=e,t[3]=O(t[3],45),s}f64(){return Number(this.next()>>11n)/2**53}},B=n=>BigInt.asUintN(64,BigInt(Math.trunc(n))),V=()=>BigInt(Math.floor(Math.random()*2**32))<<32n|BigInt(Math.floor(Math.random()*2**32)),S=class extends Error{},F=class extends Error{constructor(t){super("exit "+t),this.code=t}};var M=class{constructor(t={}){this.it=new z,this.args=[],this.ow=!1,this.mem=[],this.nums=[],this.scripts=[],this.lists=[],this.repl=new Map,this.replNum=!1,this.gen=1,this.bound=Q,this.frames=[],this.units=new Map,this.unitIds=new Map,this.rng=null,this.fs=[],this.touched=[],this.lmap=new Map,this.site=[-1,0],this.ops=0,this.lib=t.lib||{},this.vfs=t.vfs||new Map,this.write=t.write||(s=>{typeof process<"u"&&process.stdout.write(s)}),this.handles=new Map,this.hostWord=null,this.stdin=t.stdin||null,this.shell=t.shell||null,this.natives=new Map,this.reads=null,this.writes=null,this.tracking=!1,this.lastInterp=null,this.laneAt=null,this.laneStack=[],this.lanesJoined=null,this.laneView=null,this.onJoin=null,this.B={n:1/0},vt(this)}setArgv(t){this.lists[ct]=t.map(p),this.nset(_t,t.length)}native(t,s){this.natives.set(t,s);let e={val:t,code:{n:s},gen:0,nat:t};this.repl.has(t)||this.repl.set(t,[]),this.repl.get(t).unshift(e)}fail(t){throw new S(this.laneAt!==null?`lane ${this.laneAt}: ${t}`:t)}pop(t){return this.args.length||this.fail(`${t}: stack is empty`),this.args.pop()}name(t){let s=this.it.name(t),e=s.indexOf("");return e<0?s:s.slice(0,e)}l(t){return this.it.local[t]?this.localSym(t):t}localSym(t){let s=Math.max(0,this.fs.length-1),e=t+","+s,i=this.lmap.get(e);return i===void 0&&(i=this.it.sym(this.it.name(t)+""+s),this.it.local[i]=!1,this.lmap.set(e,i)),this.touch(s,i),i}touch(t,s){for(;this.touched.length<=t;)this.touched.push([]);this.touched[t].includes(s)||this.touched[t].push(s)}clearLocals(t){if(t<this.touched.length&&this.touched[t].length){for(let s of this.touched[t])this.mem[s]=void 0,this.nums[s]=void 0,this.lists[s]=void 0,this.scripts[s]=void 0;this.touched[t]=[]}}visible(t){let s=this.repl.get(t);if(!s)return null;for(let e=s.length-1;e>=0;e--)if(s[e].gen<this.bound)return s[e];return null}tryExpand(t){let s=this.visible(t);if(!s)return!1;if(!s.code)this.args.push(s.val);else if(s.code.n){let e=this.bound;this.bound=s.gen,s.code.n(this),this.bound=e}else this.fs.push({body:{ops:s.code.u},pc:0,bound:s.gen,lib:s.lib});return!0}wordRc(t,s,e){if(e!==null){if(this.replNum&&this.tryExpand(t))return;this.args.push(e);return}this.preWord&&this.preWord(t)||this.repl.size&&this.tryExpand(t)||this.hostWord&&this.hostWord(this,t)||this.args.push({w:t,k:s})}cmdSite(t){return this.repl.size&&this.tryExpand(t)?!1:this.cmd(t)}popTo(t){t=this.l(t),this.args.length||this.fail(`${this.name(t)}@: stack is empty`),this.mset(t,this.args.pop())}need(t,s){this.args.length<t&&this.fail(`${s}: needs ${t} args, stack has ${this.args.length}`)}t(t){return u(this.args[t])}optT(t){return t<this.args.length?u(this.args[t]):null}x(t){return g(this.args[t])}argSym(t){let s=this.args[t],e=typeof s=="object"?s.k:this.it.sym(u(s));return this.l(e)}dest(t,s){return this.args.length>t?this.argSym(t):s}nset(t,s){this.nums[t]=s,this.writes&&this.writes.add(t)}mset(t,s){this.mem[t]=s,this.writes&&this.writes.add(t)}wrote(t){this.writes&&this.writes.add(t)}read(t){this.reads&&this.reads.add(t)}getVal(t){return this.reads&&this.reads.add(t),this.mem[t]!==void 0?this.mem[t]:this.nums[t]!==void 0?this.nums[t]:t===ft?String(V()):"404"}interp(t){return u(this.getVal(this.l(t)))}lookup(t){return t=this.l(t),!this.replNum&&this.mem[t]===void 0&&this.nums[t]!==void 0?(this.read(t),this.args.push(this.nums[t]),!1):this.dispatch(this.getVal(t))}dispatch(t){let s=null;if(typeof t=="number"){if(this.replNum&&this.tryExpand(C(t)))return!1}else if(typeof t=="object"){if(this.repl.size&&this.tryExpand(t.w)||this.hostWord&&this.hostWord(this,t.w,!0))return!1}else s=t;if(s!==null&&s&&!dt(s)){let e=W(s);if(e)return this.cmdSite(e);if(pt(s)||s[0]==="\u27E8"){let i;if(s[0]==="\u27E8"){let r;try{r=H(s,this.it)}catch{r=null}if(!r||r.ops.length!==1)return this.args.push(t),!1;i=r.ops[0]}else i=k(s,this.it);return this.step(i,!1)==="done"}if(this.repl.size&&this.tryExpand(s)||this.hostWord&&this.hostWord(this,s,!0))return!1}return this.args.push(t),!1}joined(){return this.args.map(u).join(" ")}setMem(t){t=this.l(t);let s=this.args.length===1?this.args.pop():p(this.joined());this.args=[],this.mset(t,s)}setNum(t){t=this.l(t);let s=this.args.length?g(this.args[0]):0;this.args=[],this.nset(t,s)}incBy(t,s){t=this.l(t);let e=1;this.args.length&&(e=g(this.args[0]),this.args=[]);let i=this.nums[t]??0;this.nset(t,i+s*e)}codeOf(t){let s=this.units.get(t);return s||(s=this.reader?this.reader(t):H(t,this.it),s.unit=this.unitIds.has(t)?this.unitIds.get(t):-1,this.units.size>4096&&this.units.clear(),this.units.set(t,s),s)}defScript(t){t=this.l(t);let s=this.joined().replace(/\|/g,"~");this.args=[];let e;try{e=this.codeOf(s)}catch(i){this.fail(`${this.name(t)}$: ${i.message}`)}this.scripts[t]={u:e,b:this.gen}}libTexts(){return this._libTexts||(this._libTexts=new Set(Object.values(this.lib)))}inLib(){let t=this.fs[this.fs.length-1];return!!(t&&t.lib)}libSrc(t){let s=t.replace(/^\.\//,"");return this.lib[s]??null}fileText(t){let s=t.replace(/^\.\//,"");return this.vfs.has(s)?this.vfs.get(s):this.libSrc(s)}frameFor(t){let s=this.scripts[t];if(s)return s.n?(s.n(this),null):{body:{ops:s.u},pc:0,bound:s.b};let e=this.name(t);if(this.fileFrame&&this.fileText(e)!==null)return this.fileFrame(e);this.fail(`${e}.: no script or file named '${e}'`)}call(t){let s=this.frameFor(t);s&&this.fs.push(s)}count(t,s){let e=this.args[t],i=typeof e=="number"?e:D(u(e).trim())??NaN;if(Number.isFinite(i)&&i>=0&&Number.isInteger(i))return i;this.fail(`${s}: repeat count must be a whole number >= 0, got '${u(e)}'`)}repeat(t){let s=this.count(0,this.name(t)+".");this.fs.push({body:{rep:{k:t,i:0,n:s,amp:!1}},pc:0,bound:this.bound})}callSite(t){t=this.l(t),this.args.length===1?this.repeat(t):this.call(t)}srcFrame(t){let s;try{s=this.codeOf(t)}catch(e){this.fail("parse: "+e.message)}return{body:{ops:s},pc:0,bound:Q}}runSrc(t){this.fs.push(this.srcFrame(t))}fileFrame(t){let s=this.fileText(t);s===null&&this.fail(`${t}: No such file or directory (os error 2)`);let e=this.srcFrame(s);return this.libTexts().has(s)&&(e.lib=!0),e}hostFrame(t){return{body:{host:t},pc:0,bound:this.bound}}guarded(t){try{return t(this),null}catch(s){return this.statusOf(s)}}statusOf(t){let s;if(t instanceof S)s={error:t.message};else if(t instanceof F)s={exit:t.code};else if(t instanceof RangeError)s={error:"too deep: "+t.message};else throw t;for(this.resetFrames(),this.laneView&&this.endView();this.laneStack.length;)this.useState(this.laneStack.pop().fork);return this.laneAt=null,s}resetFrames(){for(;this.fs.length;)this.fs.pop(),this.clearLocals(this.fs.length)}runText(t,s=1/0){let e=this.guarded(i=>i.runSrc(t));return e||this.run(s)}run(t=1/0){t>Number.MAX_SAFE_INTEGER&&(t=Number.MAX_SAFE_INTEGER);let s={n:t},e=this.B;this.B=s;try{let i=this.runTo(0);return this.ops+=t-s.n,i}catch(i){return this.ops+=t-s.n,this.statusOf(i)}finally{this.B=e}}runTo(t){for(;;)try{return this.runInner(this.B,t)}catch(s){if(!(s instanceof S)||!this.recover||!this.recover(s,t))throw s}}recover(t,s){let e=this.fs.length-1;for(;e>=s&&!this.fs[e].catch;)e--;if(e<s)return!1;let i=this.fs[e].catch;for(;this.fs.length>e;)this.fs.pop(),this.clearLocals(this.fs.length);for(this.frames.length=Math.min(this.frames.length,i.frames);this.laneStack.length>i.lanes;)this.useState(this.laneStack.pop().fork);return this.laneAt=i.laneAt,this.args=[],this.mset(G,p(t.message)),this.runSrc(i.h),!0}st(t){this.fs.length>t+1&&this.runTo(t+1),this.bound=this.fs[t].bound}endFrame(t){this.clearLocals(t),this.fs.splice(t,1),t<this.touched.length&&this.touched.splice(t,1)}tailInto(t,s){this.clearLocals(t);let e=this.frameFor(s);if(e)if(this.fs.length>t+1){let i=this.fs.splice(t+1);this.fs[t]=e,this.fs.push(...i)}else this.fs[t]=e;else this.endFrame(t)}canFastTail(t){return this.fs.length===t+1&&!(this.touched[t]&&this.touched[t].length)}runInner(t,s=0){for(;;){let e=this.fs.length;if(e<=s)return"done";if(t.n<=0)return"ran";let i=e-1,l=this.fs[i];this.bound=l.bound;let r=l.body;if(r.ops&&r.ops.fn){t.n--;let o=r.ops.fn(this,i);o&&o.tail!==void 0?this.tailInto(i,o.tail):this.endFrame(i)}else if(r.ops){if(this.interpret(t,i,l))return"ran"}else r.rep?(t.n--,this.repStep(i,r.rep)):r.host&&(t.n--,this.endFrame(i),r.host(this))}}interpret(t,s,e){let i=e.body.ops,l=i.ops.length,r=e.pc;for(;;){let o="end",a=0;for(;r<l;){if(t.n<=0)return this.fs[s].pc=r,!0;t.n--,this.site=[i.unit,r],this.siteCode=i;let c=i.ops[r];r++;let h=this.step(c,r===l);if(h==="done"){this.fs.length=s+1,o="end";break}if(h&&h.tail!==void 0){o="tail",a=h.tail;break}if(this.fs.length!==s+1){o="pushed";break}}if(o==="end")this.endFrame(s);else if(o==="pushed")r===l?this.endFrame(s):this.fs[s].pc=r;else{if(this.canFastTail(s)){let c=this.scripts[a];if(c&&c.u){this.fs[s].body={ops:c.u},this.fs[s].bound=c.b,this.fs[s].pc=0,this.bound=c.b,i=c.u,l=i.ops.length,r=0;continue}}this.tailInto(s,a)}return!1}}repStep(t,s){if(s.i<s.n){let e=s.i;s.i++,s.amp&&this.nset(rt,e),this.args=[],this.call(s.k)}else this.args=[],this.endFrame(t)}step(t,s){switch(t[0]){case"push":this.args.push(t[1]);break;case"interp":this.interpOp(t[1]);break;case"lanes":this.lanesOp(t[1]);break;case"word":this.wordRc(t[1],t[2],t[3]);break;case"cmd":if(this.cmdSite(t[1]))return"done";break;case"native":if(this.cmd(t[1]))return"done";break;case"pop":this.popTo(t[1]);break;case"lookup":if(this.lookup(t[1]))return"done";break;case"def":this.defScript(t[1]);break;case"setmem":this.setMem(t[1]);break;case"setnum":this.setNum(t[1]);break;case"inc":this.incBy(t[1],1);break;case"dec":this.incBy(t[1],-1);break;case"call":{let e=this.l(t[1]);if(s)return this.args.length===1?(this.repeat(e),null):{tail:e};this.args.length===1?this.repeat(e):this.call(e);break}case"callpass":{let e=this.l(t[1]);if(s)return{tail:e};this.call(e);break}case"cond":if(this.ow===t[1])return this.step(t[2],s);break}return null}interpOp(t){let s=this.reads,e=this.tracking?new Set:null;e&&(this.reads=e);let i="";for(let r of t)i+=r[0]==="T"?r[1]:this.interp(r[1]);this.reads=s;let l=p(i);if(this.args.push(l),e){if(s)for(let r of e)s.add(r);this.lastInterp={v:l,parts:t,reads:e,at:this.args.length}}}state(){return{mem:this.mem,nums:this.nums,lists:this.lists,scripts:this.scripts,repl:this.repl}}useState(t){this.mem=t.mem,this.nums=t.nums,this.lists=t.lists,this.scripts=t.scripts,this.repl=t.repl}copyState(t){return{mem:t.mem.slice(),nums:t.nums.slice(),lists:t.lists.map(s=>s&&s.slice()),scripts:t.scripts.slice(),repl:new Map([...t.repl].map(([s,e])=>[s,e.slice()]))}}laneWrites(t,s){let e=new Set,i=new Set,l=Math.max(t.mem.length,t.nums.length,t.lists.length,t.scripts.length);for(let r=0;r<l;r++){let o=t.lists[r],a=s.lists[r],c=o!==a&&(!o||!a||o.length!==a.length||o.some((h,_)=>h!==a[_]));(t.mem[r]!==s.mem[r]||t.nums[r]!==s.nums[r]||t.scripts[r]!==s.scripts[r]||c)&&e.add(r)}for(let[r,o]of t.repl){let a=s.repl.get(r);(!a||a.length!==o.length||o.some((c,h)=>c!==a[h]))&&i.add(r)}return{names:e,repl:i}}lanesOp(t){let s=this.args,e={L:t,fork:this.state(),lanes:[],laneAt:this.laneAt,view:this.laneView,ow0:this.ow};this.endView(),this.laneStack.push(e);let i=t.spread!=null?(this.lists[this.l(t.spread)]||[]).slice():null,l=i?i.length:t.lanes.length,r=this.bound,o=-1,a=()=>{if(o>=0){let c=this.state();e.lanes[o]={args:this.args,st:c,w:this.laneWrites(c,e.fork)},this.useState(e.fork)}if(o++,o>=l){this.joinLanes(e,s);return}this.fs.push(this.hostFrame(a)),this.args=i?[...s,i[o]]:s.slice(),this.useState(this.copyState(e.fork)),this.laneAt=o,this.ow=e.ow0,this.fs.push({body:{ops:i?t.shared:t.lanes[o]},pc:0,bound:r})};this.fs.push(this.hostFrame(a)),this.args=[]}joinLanes(t,s){this.useState(t.fork),this.ow=t.ow0,this.laneStack.pop(),this.laneAt=t.laneAt,this.args=s,this.lanesJoined=t,this.onJoin&&this.onJoin(t)}joinedLanes(t){return this.lanesJoined||this.fail(`${t}: no joined lanes`),this.lanesJoined.lanes}viewLane(t){this.endView();let s=this.joinedLanes("lane")[t];s||this.fail(`lane: no lane ${t}, there are ${this.lanesJoined.lanes.length}`);let e=[];for(let l of s.w.names)e.push([l,this.mem[l],this.nums[l],this.lists[l],this.scripts[l]]),this.mem[l]=s.st.mem[l],this.nums[l]=s.st.nums[l],this.lists[l]=s.st.lists[l],this.scripts[l]=s.st.scripts[l];let i=[];for(let l of s.w.repl)i.push([l,this.repl.get(l)]),this.repl.set(l,s.st.repl.get(l));this.laneView={lane:s,saved:e,savedR:i}}endView(){let t=this.laneView;if(!t)return;let s=t.lane.st;for(let[e,i,l,r,o]of t.saved)this.mem[e]===s.mem[e]&&(this.mem[e]=i),this.nums[e]===s.nums[e]&&(this.nums[e]=l),this.lists[e]===s.lists[e]&&(this.lists[e]=r),this.scripts[e]===s.scripts[e]&&(this.scripts[e]=o);for(let[e,i]of t.savedR)this.repl.get(e)===s.repl.get(e)&&(i?this.repl.set(e,i):this.repl.delete(e));this.laneView=null}onErr(t,s){if(this.args.length>t){let e=this.t(t);this.args=[],this.mset(G,p(s)),this.runSrc(e)}else this.fail(s)}list(t){return this.lists[t]||(this.lists[t]=[]),this.lists[t]}seedOf(){return this.args.length?this.args.length===1?[B(this.x(0)),0n]:[B(this.x(0)),B(this.x(1))]:null}handle(t){let s=this.handles.get(t);if(!s)throw new Error("no open file");return s}cmd(t){let s=xt[t];s||this.fail(`${t}: not part of this page`);let e=s.call(this,t);return e!==void 0?e:(this.args=[],!1)}};function v(n){this.need(2,n);let t=this.x(0),s=this.x(1),e=n==="+"?t+s:n==="-"?t-s:n==="*"?t*s:n==="/"?t/s:n==="^"?Math.pow(t,s):t%s;n==="%"&&e<0&&(e+=Math.abs(s)),this.nset(this.dest(2,L),e)}function w(n){this.need(2,n);let t=this.args[0],s=this.args[1];this.ow=n==="="?j(t,s):n==="!="?!j(t,s):n===";"?this.ow&&!j(t,s):n==="&"?this.ow&&j(t,s):n==="=="?g(t)===g(s):n==="<<"?g(t)<g(s):n===">>"?g(t)>g(s):n==="<<="?g(t)<=g(s):g(t)>=g(s)}function y(n){this.fail(`${n}: threads and the network are not part of the browser machine`)}function P(n){let t=n.seedOf();return t?n.rng=new E(t[0],t[1]):n.rng||(n.rng=new E(V(),V())),n.rng}var xt={">"(){this.write(`
`+this.args.map(n=>" "+u(n)).join(""))},">_"(){this.write(this.args.map(u).join(""))},"<"(n){this.need(1,n);let t=this.t(0);this.shell||this.fail("<: no shell in the browser");let s;try{s=this.shell(t)}catch(e){this.fail(`<: ${t}: ${e.message}`)}this.mset(lt,p(s.out)),this.nset(ot,s.code)},"<_"(){let n=this.dest(0,ht),t=this.stdin?this.stdin():null;this.mset(n,p(t===null?"":t)),this.mset(Z,t===null?1:0)},w(n){this.need(2,n),this.vfs.set(this.t(0),this.t(1))},"+w"(n){this.need(2,n),this.vfs.set(this.t(0),(this.vfs.get(this.t(0))||"")+this.t(1))},mv(n){this.need(2,n);let t=this.t(0),s=this.t(1);this.vfs.has(t)||this.fail(`mv: ${t} ${s}: No such file or directory (os error 2)`),this.vfs.set(s,this.vfs.get(t)),this.vfs.delete(t)},r(n){this.need(1,n);let t=this.t(0),s=this.args.length>1?this.argSym(1):this.argSym(0),e=this.fileText(t);e===null&&this.fail(`r: ${t}: No such file or directory (os error 2)`),this.mset(s,p(e))},"r@"(n){this.need(4,n);let t=this.t(0),s=this.x(1),e=this.x(2),i=this.argSym(3),l=this.fileText(t);l===null&&this.fail(`r@: ${t}: No such file or directory (os error 2)`);let r=new TextEncoder().encode(l).slice(s,s+e);this.mset(i,p(new TextDecoder().decode(r)))},"?)"(n){this.need(2,n);let t=this.t(0),s=this.t(1),e=this.optT(2);this.args=[];let i=t.split(/\s+/).filter(Boolean),l=null;for(let o=0;o+1<i.length;o+=2)if(i[o]===s){l=i[o+1];break}let r=l??e;return r!==null&&this.runSrc(r),!1},"&&&"(n){this.need(2,n);let t=this.count(0,"&&&"),s=this.argSym(1);this.fs.push({body:{rep:{k:s,i:0,n:t,amp:!0}},pc:0,bound:this.bound})},"***":y,t:y,"-t":y,"<t":y,">t":y,tcp:y,n(n){this.need(2,n);let t=this.x(0);this.nset(this.argSym(1),t)},"+":v,"-":v,"*":v,"/":v,"^":v,"%":v,"!!"(){this.ow=!this.ow},"=":w,"!=":w,";":w,"&":w,"==":w,"<<":w,">>":w,"<<=":w,">>=":w,l(n){this.need(2,n);let t=this.args.length,s=this.argSym(t-1),e=this.list(s);for(let i=0;i<t-1;i++)e.push(this.args[i]);this.wrote(s)},"-l"(n){this.need(1,n);let t=this.argSym(0),s=this.list(t);this.wrote(t),this.mset(at,s.length?s.pop():"404")},"[l]"(n){this.need(3,n);let t=this.argSym(0),s=Math.trunc(this.x(1)),e=this.argSym(2);this.read(t);let i=this.list(t);s<0&&(s+=i.length),s>=0&&s<i.length&&this.mset(e,i[s])},"l="(n){this.need(3,n);let t=this.args[0],s=this.argSym(1),e=Math.trunc(this.x(2)),i=this.list(s);this.wrote(s),e<0&&(e+=i.length),e>=0&&e<i.length&&(i[e]=t)},"l-"(n){this.need(1,n);let t=this.argSym(0);this.lists[t]=void 0,this.wrote(t)},"#l"(n){this.need(1,n);let t=this.argSym(0);this.read(t);let s=this.lists[t];this.nset(this.dest(1,L),s?s.length:0)},"/l"(n){this.need(2,n);let t=this.t(0),s=this.argSym(1),e=this.optT(2),i=e===null?t.split(/[ \n\t\r\f\v]+/).filter(Boolean):e===""?Array.from(t):t.split(e);this.lists[s]=i.map(p),this.wrote(s)},"l/"(n){this.need(2,n);let t=this.argSym(0),s=this.argSym(1),e=this.optT(2)??" ";this.read(t);let i=this.lists[t];this.mset(s,p(i?i.map(u).join(e):""))},"#s"(n){this.need(1,n);let t=Array.from(u(this.args[0])).length;this.nset(this.dest(1,L),t)},f(n){this.need(3,n);let t=this.t(0),s=this.t(2),e=this.argSym(1),i=this.vfs.get(t);if(i===void 0){if(s[0]==="r")return this.onErr(3,`f: ${t}: No such file or directory (os error 2)`),!1;i=""}s[0]==="w"&&(i=""),this.vfs.set(t,i),this.handles.set(e,{path:t,pos:s[0]==="a"?i.length:0,eof:!1})},"+f"(n){this.need(3,n);let t=this.argSym(0),s=this.x(1),e=this.argSym(2),i=this.handles.get(t);if(!i)return this.onErr(3,`+f: ${this.name(t)}: no open file`),!1;let l=this.mem[e]!==void 0?u(this.mem[e]):"",r=this.vfs.get(i.path);this.vfs.set(i.path,r.slice(0,s).padEnd(s,"\0")+l+r.slice(s+l.length))},"@f"(n){this.need(4,n);let t=this.argSym(0),s=this.x(1),e=this.x(2),i=this.argSym(3),l=this.handles.get(t);l||this.fail(`@f: ${this.name(t)}: no open file`);let r=this.vfs.get(l.path).slice(s,s+e);l.eof=r.length<e,this.mset(i,p(r))},"f-"(n){this.need(1,n),this.handles.delete(this.argSym(0))},f_(n){this.need(2,n);let t=this.argSym(0),s=this.handles.get(t);if(!s)return this.onErr(2,`f_: ${this.name(t)}: no open file`),!1;let e=this.vfs.get(s.path),i=this.t(1)+`
`;this.vfs.set(s.path,e.slice(0,s.pos)+i+e.slice(s.pos+i.length)),s.pos+=i.length},_f(n){this.need(2,n);let t=this.argSym(0),s=this.argSym(1),e=this.handles.get(t);e||this.fail(`_f: ${this.name(t)}: no open file`);let i=this.vfs.get(e.path);if(e.pos>=i.length){e.eof=!0,this.mset(s,"");return}let l=i.indexOf(`
`,e.pos);l<0&&(l=i.length);let r=i.slice(e.pos,l);e.pos=Math.min(i.length,l+1),r.endsWith("\r")&&(r=r.slice(0,-1)),this.mset(s,p(r))},"?eof"(n){this.need(1,n);let t=this.handles.get(this.argSym(0));this.mset(Z,!t||t.eof?1:0)},"#prng"(){let n=P(this);this.nset(this.dest(2,X),n.f64())},"`prng"(){let n="abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",t=P(this),s=this.dest(2,X),e="";for(let i=0;i<32;i++)e+=n[Number(t.next()%52n)];this.mset(s,p(e))},"><"(n){this.need(2,n);let t=this.t(1),s=this.args[0],e=null;try{e=this.codeOf(u(s))}catch{}J(t)!==null&&(this.replNum=!0);let i=this.gen++;this.repl.has(t)||this.repl.set(t,[]);let l=!!(this.siteCode&&this.libTexts().has(this.siteCode.src));this.repl.get(t).push({val:s,code:e?{u:e}:null,gen:i,lib:l})},"("(n){this.need(1,n);let t=this.args.length-1,s=this.count(t,"(");this.args.pop();let e=Math.min(s,t);return this.frames.push(this.args.splice(0,t-e)),!1},")"(){let n=this.frames.pop();return n||this.fail("): no open frame"),this.args=n.concat(this.args),!1},"~"(){return this.args.length||this.fail("~: stack is empty"),this.runSrc(u(this.args.pop())),!1},"##"(){return this.args.push(this.args.length),!1},"//"(){},"<-"(){return!0},"<--"(){throw new F(this.args.length?Math.trunc(g(this.args[0])):0)}};function vt(n){let t=(s,e)=>n.native(s,e);t("lanes",s=>{s.args.push(s.joinedLanes("lanes").length)}),t("lane",s=>{let e=s.count(s.args.length-1,"lane");s.args.pop(),s.viewLane(e),s.args.push(...s.lanesJoined.lanes[e].args)}),t("endlane",s=>s.endView()),t("collect",s=>{for(let e of s.joinedLanes("collect"))s.args.push(...e.args)}),t("into",s=>{s.need(1,"into");let e=s.argSym(s.args.length-1);s.args.pop();let i=[];for(let l of s.joinedLanes("into"))i.push(...l.args);s.lists[e]=i,s.wrote(e)}),t("reduce",s=>{let e=u(s.pop("reduce")),i=s.joinedLanes("reduce");if(i.length){s.args.push(...i[0].args);for(let l=i.length-1;l>=1;l--)s.fs.push(s.srcFrame(e)),s.fs.push(s.hostFrame(()=>s.args.push(...i[l].args)))}}),t("lanewise",s=>{let e=u(s.pop("lanewise")),i=s.lanesJoined,l=s.joinedLanes("lanewise");s.endView();let r=s.args,o=s.state(),a=s.ow;s.fs.push(s.hostFrame(()=>{s.useState(o),s.args=r,s.laneAt=i.laneAt,s.ow=a}));for(let c=l.length-1;c>=0;c--)s.fs.push(s.hostFrame(()=>{let h=s.state();l[c]={args:s.args,st:h,w:s.laneWrites(h,i.fork)}})),s.fs.push(s.srcFrame(e)),s.fs.push(s.hostFrame(()=>{s.useState(l[c].st),s.args=l[c].args.slice(),s.laneAt=c,s.ow=a}));s.args=[]}),t("try",s=>{let e=u(s.pop("try")),i=u(s.pop("try"));s.args=[];let l=s.hostFrame(()=>{});l.catch={h:e,frames:s.frames.length,lanes:s.laneStack.length,laneAt:s.laneAt},s.fs.push(l),s.runSrc(i)})}var et={"std/all.s":`{std/all.s  the general library in one word: core, line, text, math, check} //
std/core.s.
std/line.s.
std/text.s.
std/math.s.
std/check.s.
`,"std/check.s":`{std/check.s  checks beside the code: each prints ok or what it got, and checked prints the tally} //
0 \\( _check.s~ 1 \\= \\) <-?
0 \\(  1 _check.s#
std/core.s.
0 _cp#  0 _cf#

{2 \\( _cw@ _cg@ 0 \\( _cp; \\) ok \\> \\)}                                        '_ckok' ><
{2 \\( _cw@ _cg@ 0 \\( _cf; \\) fail got {~_cg~} want {~_cw~} \\> \\)}          '_ckbad' ><
{_cw@ _cg@ {~_cg~} {~_cw~} same {~_cg~} {~_cw~} _ckok? _ckbad!}             'expect' ><   {got want \u2192 : ok when the same text} //
{_cw@ _cg@ {~_cg~} {~_cw~} eq {~_cg~} {~_cw~} _ckok? _ckbad!}               'expectn' ><  {got want \u2192 : ok when equal as numbers} //
{_cq@ {~_cq~} \\~ yes? no! 1 0 drop? nip! 1 _ckb@ _cka@ {~_cka~} {~_ckb~} _ckok? _ckbad!}   'assert' ><  {\u2026 q \u2192 \u2026 : ok when q leaves ow true} //
{0 \\( checked _cp~ ok _cf~ failed \\> \\) 0 _cp# 0 _cf#}                   'checked' ><  {: print the tally and start a new one} //

\\)
`,"std/core.s":`{std/core.s  stack words, line arithmetic, conditions, quotations, loops} //
0 \\( _core.s~ 1 \\= \\) <-?
0 \\(  1 _core.s#

{stack} //
{_a@}                                  'drop' ><   {a \u2192} //
{_a@ {~_a~} {~_a~}}                    'dup' ><    {a \u2192 a a} //
{_a@ _b@ {~_a~} {~_b~}}                'swap' ><   {a b \u2192 b a} //
{_a@ _b@ {~_b~} {~_a~} {~_b~}}         'over' ><   {a b \u2192 a b a} //
{_c@ _b@ _a@ {~_b~} {~_c~} {~_a~}}     'rot' ><    {a b c \u2192 b c a} //
{_a@ _b@ {~_a~}}                       'nip' ><    {a b \u2192 b} //
{_b@ _a@ {~_b~} {~_a~} {~_b~}}         'tuck' ><   {a b \u2192 b a b} //
{\\//}                                  'clear' ><  {\u2026 \u2192} //
{\\##}                                  'depth' ><  {\u2026 \u2192 \u2026 n} //

{line arithmetic: the top two values become one, the rest of the line stays} //
{2 \\( _R \\+ _R~ \\)}                    'add' ><    {a b \u2192 a+b} //
{2 \\( _R \\- _R~ \\)}                    'sub' ><    {a b \u2192 a-b} //
{2 \\( _R \\* _R~ \\)}                    'mul' ><    {a b \u2192 a\xD7b} //
{2 \\( _R \\/ _R~ \\)}                    'div' ><    {a b \u2192 a\xF7b} //
{2 \\( _R \\% _R~ \\)}                    'mod' ><    {a b \u2192 a mod b} //
{2 \\( _R \\^ _R~ \\)}                    'pow' ><    {a b \u2192 a^b} //
{1 \\( -1 _R \\* _R~ \\)}                 'neg' ><    {a \u2192 -a} //
{1 \\( 1 _R \\+ _R~ \\)}                  'inc' ><    {a \u2192 a+1} //
{1 \\( 1 _R \\- _R~ \\)}                  'dec' ><    {a \u2192 a-1} //
{1 \\( _a@ {~_a~} 0 \\<< {~_a~} neg? \\)}                                    'abs' ><  {a \u2192 |a|} //
{2 \\( _b@ _a@ {~_a~} {~_b~} \\<< {~_a~} {~_b~} drop? nip! \\)}              'min' ><  {a b \u2192 smaller} //
{2 \\( _b@ _a@ {~_a~} {~_b~} \\>> {~_a~} {~_b~} drop? nip! \\)}              'max' ><  {a b \u2192 larger} //

{conditions: the top two values are compared and removed, ow holds the answer} //
{2 \\( \\<< \\)}                          'lt' ><     {a b \u2192 : ow = a<b} //
{2 \\( \\>> \\)}                          'gt' ><     {a b \u2192 : ow = a>b} //
{2 \\( \\<<= \\)}                         'le' ><     {a b \u2192 : ow = a\u2264b} //
{2 \\( \\>>= \\)}                         'ge' ><     {a b \u2192 : ow = a\u2265b} //
{2 \\( \\== \\)}                          'eq' ><     {a b \u2192 : ow = a=b as numbers} //
{2 \\( \\== \\!! \\)}                      'ne' ><     {a b \u2192 : ow = a\u2260b as numbers} //
{2 \\( \\= \\)}                           'same' ><   {a b \u2192 : ow = same text} //
{2 \\( \\!= \\)}                          'differ' >< {a b \u2192 : ow = different text} //
{\\!!}                                  'not' ><    {: ow = not ow} //
{0 \\( 0 0 \\== \\)}                      'yes' ><    {: ow = true} //
{0 \\( 1 0 \\== \\)}                      'no' ><     {: ow = false} //

{quotations: a brace literal is code waiting; ~ runs it on the line} //
{\\~}                                   'call' ><   {\u2026 q \u2192 \u2026} //
{drop? nip! \\~}                        'if' ><     {\u2026 t f \u2192 \u2026 : run t when ow, else f} //
{{} if}                                'when' ><   {\u2026 q \u2192 \u2026 : run q when ow} //
{{} swap if}                           'unless' >< {\u2026 q \u2192 \u2026 : run q unless ow} //
{_t@ \\## _d@ 0 \\( {~_d~} 0 \\>> \\) dup? 1 0 drop? nip! \\( {~_t~} \\~ \\// \\)}   'test' ><   {\u2026 q \u2192 \u2026 : run q on a copy of the top value; only its ow remains} //
{_q2@ _q1@ {~_q2~} {~_q1~} test 1 \\( when \\// \\)}    'and' ><  {p q \u2192 : ow = p and q, q tested only after p} //
{_q2@ _q1@ {~_q2~} {~_q1~} test 1 \\( unless \\// \\)}  'or' ><   {p q \u2192 : ow = p or q} //
{_q@ _x@ 0 \\( {~_x~} _dip \\l \\) {~_q~} \\~ 0 \\( _dip \\-l \\) {~$~}}       'dip' ><   {\u2026 x q \u2192 \u2026 x : run q under x} //
{_q@ dup {~_q~} dip}                                                   'keep' ><  {x q \u2192 q(x) x} //
{_q@ _p@ {~_q~} swap {~_p~} keep rot \\~}                               'bi' ><    {x p q \u2192 p(x) q(x)} //

{loops: control state lives in lists, so loops nest} //
{0 \\( _tn -1 _k \\[l] {~_k~} 0 \\>> \\) _times1..?} _times$
{0 \\( {~_k~} 1 _K \\- {~_K~} _tn -1 \\l= _tq -1 _c \\[l] {~_c~} \\) \\~ _times..} _times1$
{_n@ _q@ 0 \\( {~_q~} _tq \\l {~_n~} _tn \\l \\) _times.. 0 \\( _tq \\-l _tn \\-l \\)}    'times' ><  {\u2026 q n \u2192 \u2026 : run q n times on the line} //

{0 \\( _wc -1 _c \\[l] {~_c~} \\) test _while1..?} _while$
{0 \\( _wb -1 _c \\[l] {~_c~} \\) \\~ _while..} _while1$
{_b@ _c@ 0 \\( {~_c~} _wc \\l {~_b~} _wb \\l \\) _while.. 0 \\( _wc \\-l _wb \\-l \\)}    'while' ><  {\u2026 c b \u2192 \u2026 : while c tests true run b} //

{_c@ _b@ 0 \\( {~_b~} _uq \\l {~_c~} _uc \\l \\) _untilgo.. 0 \\( _uq \\-l _uc \\-l \\)}  'until' ><  {\u2026 b c \u2192 \u2026 : run b, then again until c tests true} //
{0 \\( _uq -1 _c \\[l] {~_c~} \\) \\~ 0 \\( _uc -1 _c \\[l] {~_c~} \\) test _untilgo..!} _untilgo$

{private names for the library's own quotations, which run where they are called} //
{_a@ {~_a~} {~_a~}}                    '_dup' ><
{_a@ _b@ {~_a~} {~_b~}}                '_swap' ><
{_a@ _b@ {~_b~} {~_a~} {~_b~}}         '_over' ><
{2 \\( _R \\+ _R~ \\)}                    '_add' ><
{2 \\( _R \\* _R~ \\)}                    '_mul' ><
{1 \\( 1 _R \\+ _R~ \\)}                  '_inc' ><
{2 \\( _b@ _a@ {~_a~} {~_b~} \\<< {~_a~} {~_b~} drop? nip! \\)}              '_min' ><
{2 \\( _b@ _a@ {~_a~} {~_b~} \\>> {~_a~} {~_b~} drop? nip! \\)}              '_max' ><
{2 \\( _R \\- _R~ \\)}                    '_sub' ><
{2 \\( _R \\/ _R~ \\)}                    '_div' ><

\\)
`,"std/datum.s":`{std/datum.s  datum cubes: a value of 24 bits, red green blue, and its bits by number} //
std/core.s.
0 \\( _datum.s~ 1 \\= \\) <-?
0 \\(  1 _datum.s#

{bytes} //
{dup 65536 mod sub 65536 div}          'red' ><    {v \u2192 red byte} //
{65536 mod dup 256 mod sub 256 div}    'green' ><  {v \u2192 green byte} //
{256 mod}                              'blue' ><   {v \u2192 blue byte} //
{swap 256 mul add swap 65536 mul add}  'rgb' ><    {r g b \u2192 v} //

{bits} //
{2 swap pow div dup 1 mod sub 2 mod}   'bitof' ><  {v n \u2192 bit n of v} //
{_n@ _v@ {~_v~} {~_v~} {~_n~} bitof 2 mul neg inc 2 {~_n~} pow mul add}   'flip' ><   {v n \u2192 v with bit n toggled} //
`,"std/gem.s":`{std/gem.s  gematria: ordinal, reduced and reverse ordinal sums over letters a-z} //
0 \\( _gem.s~ 1 \\= \\) <-?
0 \\(  1 _gem.s#
std/text.s.

{a 1 b 2 c 3 d 4 e 5 f 6 g 7 h 8 i 9 j 10 k 11 l 12 m 13 n 14 o 15 p 16 q 17 r 18 s 19 t 20 u 21 v 22 w 23 x 24 y 25 z 26} _ordt\`
{a 1 b 2 c 3 d 4 e 5 f 6 g 7 h 8 i 9 j 1 k 2 l 3 m 4 n 5 o 6 p 7 q 8 r 9 s 1 t 2 u 3 v 4 w 5 x 6 y 7 z 8} _redt\`
{a 26 b 25 c 24 d 23 e 22 f 21 g 20 h 19 i 18 j 17 k 16 l 15 m 14 n 13 o 12 p 11 q 10 r 9 s 8 t 7 u 6 v 5 w 4 x 3 y 2 z 1} _rordt\`

{_gc@ 0 \\( {~_gt~} {~_gc~} '\\0' \\?) \\)}                      '_gval' ><
{1 \\( lower chars {_gval} map sum \\)}                         '_gsum' ><
{0 \\( {~_ordt~} _gt\` \\) _gsum}                                 'ord' ><   {text \u2192 ordinal sum, a=1 \u2026 z=26} //
{ord}                                                          '_gord' ><
{0 \\( {~_redt~} _gt\` \\) _gsum}                                 'red' ><   {text \u2192 reduced sum, a=1 \u2026 i=9 j=1 \u2026} //
{0 \\( {~_rordt~} _gt\` \\) _gsum}                                'rord' ><  {text \u2192 reverse ordinal sum, a=26 \u2026 z=1} //
{_gw@ {~_gw~} {~_gw~} ord {~_gw~} red {~_gw~} rord}         'gem' ><   {text \u2192 text ord red rord} //
{1 \\( words {_gord} map \\)}                                      'ords' ><  {text \u2192 ord of each word} //

\\)
`,"std/line.s":`{std/line.s  the line as an array: every word here acts on the whole line; k ( \u2026 ) narrows it} //
0 \\( _line.s~ 1 \\= \\) <-?
0 \\(  1 _line.s#
std/core.s.

{moving the line in and out of lists; _lv numbers nested uses} //
{\\## _d@ 0 \\( {~_d~} 0 \\>> \\) _drain1..?} _drain$
{_v@ 0 \\( {~_v~} {_dl~_lv~} \\l \\) _drain..} _drain1$

{0 \\( {_dl~_lv~} _N \\#l {~_N~} 0 \\>> \\) _pour1..?} _pour$
{0 \\( {_dl~_lv~} \\-l _gk -1 _c \\[l] {~$~} {~_c~} \\l \\) _pour..} _pour1$
{_k@ 0 \\( _lv; {_dl~_lv~} \\l- {~_k~} \\l- {~_k~} _gk \\l \\) _drain.. _pour.. 0 \\( _gk \\-l _lv: \\)}   'gather' ><  {\u2026 name \u2192 : the line becomes list name} //

{0 \\( _sn -1 _c \\[l] _si -1 _i \\[l] {~_c~} _N \\#l {~_i~} {~_N~} \\<< \\) _spill1..?} _spill$
{0 \\( _sn -1 _c \\[l] _si -1 _i \\[l] {~_c~} {~_i~} _x \\[l] {~_i~} 1 _I \\+ {~_I~} _si -1 \\l= \\) {~_x~} _spill..} _spill1$
{_k@ 0 \\( {~_k~} _sn \\l 0 _si \\l \\) _spill.. 0 \\( _sn \\-l _si \\-l \\)}   'spill' ><  {name \u2192 \u2026 : list name onto the line, list kept} //

{_shw gather 0 \\( _shw spill \\> \\) _shw spill}       'show' ><   {\u2026 \u2192 \u2026 : print the line, keep it} //
{\\## _d@ \\// {~_d~}}                                  'count' ><  {\u2026 \u2192 n} //

{map, filter, fold, scan} //
{0 \\( {_dl~_lv~} _N \\#l {~_N~} 0 \\>> \\) _map1..?} _map$
{0 \\( _mq -1 _c \\[l] {_dl~_lv~} \\-l \\) {~$~} 1 \\( {~_c~} \\~ \\) _map..} _map1$
{_q@ 0 \\( _lv; {_dl~_lv~} \\l- {~_q~} _mq \\l \\) _drain.. _map.. 0 \\( _mq \\-l _lv: \\)}   'map' ><  {x\u2026 q \u2192 q(x)\u2026 : q sees one value at a time} //

{0 \\( {_dl~_lv~} _N \\#l {~_N~} 0 \\>> \\) _filter1..?} _filter$
{0 \\( _fq -1 _c \\[l] {_dl~_lv~} \\-l \\) {~$~} {~$~} 1 \\( {~_c~} \\~ \\// \\) drop! _filter..} _filter1$
{_q@ 0 \\( _lv; {_dl~_lv~} \\l- {~_q~} _fq \\l \\) _drain.. _filter.. 0 \\( _fq \\-l _lv: \\)}   'filter' ><  {x\u2026 q \u2192 x\u2026 : keep x where q leaves ow true} //

{0 \\( {_dl~_lv~} _N \\#l {~_N~} 0 \\>> \\) _fold1..?} _fold$
{0 \\( _oq -1 _c \\[l] {_dl~_lv~} \\-l \\) {~$~} {~_c~} \\~ _fold..} _fold1$
{_q@ \\## _d@ 0 \\( {~_d~} 0 \\== \\) \\<-? 0 \\( _lv; {_dl~_lv~} \\l- {~_q~} _oq \\l \\) _drain.. 0 \\( {_dl~_lv~} \\-l \\) {~$~} _fold.. 0 \\( _oq \\-l _lv: \\)}   'fold' ><  {x\u2026 q \u2192 r : q joins values left to right} //
{_q@ \\## _d@ 0 \\( {~_d~} 0 \\== \\) \\<-? 0 \\( _lv; {_dl~_lv~} \\l- \\) _drain.. {_dl~_lv~} spill 0 \\( {_dl~_lv~} \\l- _lv: \\) {_swap ~_q~} fold}   'foldr' ><  {x\u2026 q \u2192 r : q joins values right to left} //
{_q@ {_over _swap ~_q~} fold}                                                       'scan' ><  {x\u2026 q \u2192 running results} //

{0 \\( _lv; {_dl~_lv~} \\l- \\) _drain.. {_dl~_lv~} spill 0 \\( {_dl~_lv~} \\l- _lv: \\)}   'rev' ><  {a b c \u2192 c b a} //
{0 {_add} fold}                                        'sum' ><      {x\u2026 \u2192 total} //
{1 {_mul} fold}                                        'product' ><  {x\u2026 \u2192 product} //
{{_max} fold}                                          'most' ><     {x\u2026 \u2192 largest} //
{{_min} fold}                                          'least' ><    {x\u2026 \u2192 smallest} //
{1 \\( _n@ 0 {_inc _dup} {~_n~} times drop \\)}           'iota' ><     {n \u2192 1 2 \u2026 n} //
{1 \\( _n@ 0 {_dup _inc} {~_n~} times drop \\)}           'til' ><      {n \u2192 0 1 \u2026 n-1} //
{_n@ \\## _d@ 0 \\( {~_d~} {~_n~} sub 0 max _k@ \\) {~_k~} \\( _trash gather \\)}            'take' ><  {x\u2026 n \u2192 first n} //
{_n@ \\## _d@ 0 \\( {~_d~} {~_n~} sub 0 max _k@ \\) {~_k~} \\( _keep gather \\) \\// _keep spill}   'skip' ><  {x\u2026 n \u2192 all but the first n} //

{sort: insertion through the line itself} //
{0 \\( {_dl~_lv~} _N \\#l {~_N~} 0 \\>> \\) _sort1..?} _sort$
{0 \\( {_dl~_lv~} \\-l {~$~} _sx\` \\) _ins.. {~_sx~} _unst.. _sort..} _sort1$
{\\## _d@ 0 \\( {~_d~} 0 \\>> \\) _ins1..?} _ins$
{_y@ 0 \\( {~_y~} {~_sx~} \\>> \\) {~_y~} _ins2..?} _ins1$
{_y@ 0 \\( {~_y~} {_st~_lv~} \\l \\) _ins..} _ins2$
{0 \\( {_st~_lv~} _N \\#l {~_N~} 0 \\>> \\) _unst1..?} _unst$
{0 \\( {_st~_lv~} \\-l \\) {~$~} _unst..} _unst1$
{0 \\( _lv; {_dl~_lv~} \\l- {_st~_lv~} \\l- \\) _drain.. _sort.. 0 \\( _lv: \\)}   'sort' ><  {x\u2026 \u2192 ascending} //

\\)
`,"std/logic.s":`{std/logic.s  gates on bits for the logic dialect: each takes its inputs from the line and leaves one bit} //
std/core.s.
0 \\( _logic.s~ 1 \\= \\) <-?
0 \\(  1 _logic.s#

{bits} //
{abs 1 min}                            'bit' ><    {a \u2192 1 if a\u22600, else 0} //
{}                                     'buf' ><    {a \u2192 a} //
{neg inc}                              'not' ><    {a \u2192 \xACa} //
{mul}                                  'and' ><    {a b \u2192 a\u2227b} //
{add 1 min}                            'or' ><     {a b \u2192 a\u2228b} //
{add 2 mod}                            'xor' ><    {a b \u2192 a\u2295b} //
{mul neg inc}                          'nand' ><   {a b \u2192 \xAC(a\u2227b)} //
{add 1 min neg inc}                    'nor' ><    {a b \u2192 \xAC(a\u2228b)} //
{add 2 mod neg inc}                    'xnor' ><   {a b \u2192 \xAC(a\u2295b)} //
`,"std/math.s":`{std/math.s  numbers: rounding, signs, ranges, integers, means, chance} //
0 \\( _math.s~ 1 \\= \\) <-?
0 \\(  1 _math.s#
std/line.s.

{rounding} //
{1 \\( 1 _mf \\% _mf~ \\)}                                   'frac' ><    {a \u2192 the part after the point, always \u2265 0} //
{_mx@ {~_mx~} {~_mx~} frac sub}                           'floor' ><   {a \u2192 largest whole \u2264 a} //
{neg floor neg}                                           'ceil' ><    {a \u2192 smallest whole \u2265 a} //
{0.5 add floor}                                           'round' ><   {a \u2192 nearest whole, halves up} //
{_mx@ {~_mx~} 0 lt {~_mx~} ceil? floor!}                  'trunc' ><   {a \u2192 whole part toward 0} //
{_md@ _mx@ 10 {~_md~} pow _mp@ {~_mx~} {~_mp~} mul round {~_mp~} div}   'places' ><  {a n \u2192 a rounded to n decimals} //

{signs and ranges} //
{_sx@ {~_sx~} 0 gt 1 0 drop? nip! {~_sx~} 0 lt 1 0 drop? nip! sub}      'sign' ><    {a \u2192 -1, 0 or 1} //
{_mh@ _ml@ {~_ml~} max {~_mh~} min}                       'clamp' ><   {a lo hi \u2192 a held within lo\u2026hi} //
{_mh@ _ml@ _mx@ {~_mx~} {~_ml~} ge 1 0 drop? nip! {~_mx~} {~_mh~} le 1 0 drop? nip! mul 1 eq}   'within' ><  {a lo hi \u2192 : ow = lo \u2264 a \u2264 hi} //
{_mt@ _mb@ _ma@ {~_ma~} {~_mb~} {~_ma~} sub {~_mt~} mul add}   'lerp' ><    {a b t \u2192 a + (b - a) t} //
{0.5 pow}                                                 'sqrt' ><    {a \u2192 square root} //
{dup mul}                                                 'sq' ><      {a \u2192 a\xB2} //
{sq swap sq add sqrt}                                     'hypot' ><   {a b \u2192 \u221A(a\xB2 + b\xB2)} //
{3.141592653589793}                                       'pi' ><      {\u2192 \u03C0} //

{whole numbers} //
{2 mod 0 eq}                                              'even' ><    {n \u2192 : ow = n is even} //
{even not}                                                'odd' ><     {n \u2192 : ow = n is odd} //
{2 \\( {0 ne} {_mb@ _ma@ {~_mb~} {~_ma~} {~_mb~} mod} while drop abs \\)}   'gcd' ><  {a b \u2192 greatest common divisor} //
{_mc@ _mk@ {~_mk~} {~_mc~} mul abs {~_mk~} {~_mc~} gcd div}               'lcm' ><  {a b \u2192 least common multiple} //
{1 \\( iota product \\)}                                    'fact' ><    {n \u2192 n!} //
{_mn@ 0 1 {_mb@ _ma@ {~_mb~} {~_ma~} {~_mb~} add} {~_mn~} times drop}     'fib' ><  {n \u2192 the n-th Fibonacci number, 0 1 1 2 \u2026} //
{1 \\( _mn@ {~_mn~} iota {_md@ {~_mn~} {~_md~} mod 0 eq} filter \\)}   'divisors' ><  {n \u2192 each divisor, ascending} //
{1 \\( divisors count \\) 2 eq}                         'prime' ><  {n \u2192 : ow = n is prime} //
{_mt@ _mf@ {~_mt~} {~_mf~} sub inc til {_ri@ {~_ri~} {~_mf~} add} map}   'range' ><   {a b \u2192 a a+1 \u2026 b} //

{the whole line} //
{\\## _mn@ sum {~_mn~} div}                                'mean' ><    {x\u2026 \u2192 the mean} //
{sort _mv gather 0 \\( _mv _mn \\#l \\) _mn~ 1 sub 2 div _mi@ 0 \\( _mv {~_mi~} floor _ma \\[l] _mv {~_mi~} ceil _mb \\[l] _mv \\l- \\) {~_ma~} {~_mb~} add 2 div}   'median' ><  {x\u2026 \u2192 the middle value, or the mean of the two middle ones} //

{chance: one generator for the run; seed it to repeat a run} //
{0 \\( \\#prng rn~ \\)}                                      'rand' ><    {\u2192 a float in [0, 1)} //
{1 \\( _mn@ rand {~_mn~} mul floor inc \\)}                 'dice' ><    {n \u2192 a whole number 1 \u2026 n} //
{1 \\( \\#prng \\// \\)}                                      'seed' ><    {n \u2192 : the generator starts again from n} //

\\)
`,"std/sh.s":`{std/sh.s  pipelines over lines of text: the line holds one value per text line, |> is a separator you can read along} //
0 \\( _sh.s~ 1 \\= \\) <-?
0 \\(  1 _sh.s#
std/text.s.

{}                                                  '|>' ><     {: separator, does nothing} //
{}                                                  '~>' ><     {: the same separator after a script body turns | into ~} //
{_p@ 0 \\( {~_p~} _fc \\r \\) {~_fc~} lines}          'cat' ><    {path \u2192 line\u2026} //
{_c@ 0 \\( {~_c~} \\< \\) {~r~} lines}                 'sh' ><     {command \u2192 its output lines} //
{_g@ {~_lb~~_g~~_rb~ has} filter}                   'grep' ><   {line\u2026 text \u2192 lines that contain text} //
{_g@ {~_lb~~_g~~_rb~ has not} filter}               'grepv' ><  {line\u2026 text \u2192 lines that do not} //
{_sn@ _so@ {~_lb~~_so~~_rb~ ~_lb~~_sn~~_rb~ subst} map}   'sed' ><  {line\u2026 old new \u2192 each line with old as new} //
{take}                                              'head' ><   {line\u2026 n \u2192 first n} //
{_n@ {~_n~} \\( _tl gather \\) \\// _tl spill}         'tail' ><   {line\u2026 n \u2192 last n} //
{count}                                             'wc' ><     {line\u2026 \u2192 how many} //
{0 \\( {} _uprev\` \\) {_ux@ {~_ux~} {~_uprev~} \\!= {~_ux~} _uprev\`} filter}   'uniq' ><  {line\u2026 \u2192 without repeats next to each other} //
{0 \\( 0 _nln# \\) {_nx@ 0 \\( _nln; \\) {~_nln~  ~_nx~}} map}                  'nl' ><    {line\u2026 \u2192 numbered lines} //
{{upper} map}                                       'shout' ><  {line\u2026 \u2192 each in capitals} //
{\\## _d@ 0 \\( {~_d~} 0 \\== \\) \\<-? _sh gather 0 \\( _sh _o {~_nl~} \\l/ {~_o~~_nl~} \\>_ \\)}                        'put' ><   {line\u2026 \u2192 : print one per line} //
{_p@ _sh gather 0 \\( _sh _o {~_nl~} \\l/ {~_p~} {~_o~~_nl~} \\w \\)}              '>file' ><  {line\u2026 path \u2192 : write them to path} //
{_p@ _sh gather 0 \\( _sh _o {~_nl~} \\l/ {~_p~} {~_o~~_nl~} \\+w \\)}             '>>file' >< {line\u2026 path \u2192 : append them to path} //

\\)
`,"std/text.s":`{std/text.s  text: build, split, join, case, search, format} //
0 \\( _text.s~ 1 \\= \\) <-?
0 \\(  1 _text.s#
std/line.s.

'{' _lb\`  '}' _rb\`  {
} _nl\`

{_b@ _a@ {~_a~~_b~}}                                   'cat' ><     {a b \u2192 ab} //
{_cc gather 0 \\( _cc _o {} \\l/ \\) {~_o~}}              'concat' ><  {x\u2026 \u2192 one text, no separators} //
{_cc gather 0 \\( _cc _o \\l/ \\) {~_o~}}                 'glue' ><    {x\u2026 \u2192 one text, single spaces} //
{_j@ _cc gather 0 \\( _cc _o {~_j~} \\l/ \\) {~_o~}}      'joinwith' >< {x\u2026 sep \u2192 one text} //
{_v@ {~_lb~~_v~~_rb~}}                                 'quote' ><   {x \u2192 code that pushes x} //

{_s@ 0 \\( {~_s~} _sp \\/l \\) _sp spill}                 'words' ><   {text \u2192 word\u2026} //
{_s@ 0 \\( {~_s~} _sp {} \\/l \\) _sp spill}              'chars' ><   {text \u2192 char\u2026} //
{_s@ 0 \\( {~_s~} _sp {~_nl~} \\/l _sp -1 _x \\[l] {~_x~} {} \\= _sp? \\-l? \\) _sp spill}   'lines' ><  {text \u2192 line\u2026 : a final newline ends the last line} //
{_j@ _s@ 0 \\( {~_s~} _sp {~_j~} \\/l \\) _sp spill}      'split' ><   {text sep \u2192 part\u2026} //
{1 \\( _L \\#s {~_L~} \\)}                                'len' ><     {text \u2192 characters} //
{1 \\( chars rev concat \\)}                             'reversed' >< {text \u2192 txet} //
{_n@ _s@ 0 \\( {~_s~} quote {~_n~} times concat \\)}     'repeat' ><  {text n \u2192 texttext\u2026} //
{1 \\( words glue \\)}                                   'squish' ><  {text \u2192 single-spaced} //

{_p@ _s@ 0 \\( {~_s~} _hs {~_p~} \\/l _hs _N \\#l {~_N~} 1 \\>> \\)}              'has' ><    {text part \u2192 : ow = text contains part} //
{_r@ _p@ _s@ 0 \\( {~_s~} _hs {~_p~} \\/l _hs _o {~_r~} \\l/ \\) {~_o~}}         'subst' ><  {text old new \u2192 text with every old as new} //
{_p@ _s@ 0 \\( {~_s~} _hs {~_p~} \\/l _hs _N \\#l \\) {~_N~} dec}                 'occurs' >< {text part \u2192 how many times} //

{a 'A' b 'B' c 'C' d 'D' e 'E' f 'F' g 'G' h 'H' i 'I' j 'J' k 'K' l 'L' m 'M' n 'N' o 'O' p 'P' q 'Q' r 'R' s 'S' t 'T' u 'U' v 'V' w 'W' x 'X' y 'Y' z 'Z'} _upt\`
{A 'a' B 'b' C 'c' D 'd' E 'e' F 'f' G 'g' H 'h' I 'i' J 'j' K 'k' L 'l' M 'm' N 'n' O 'o' P 'p' Q 'q' R 'r' S 's' T 't' U 'u' V 'v' W 'w' X 'x' Y 'y' Z 'z'} _lot\`
{{~_ch~}}                                              '_chid' ><
{_ch@ 0 \\( {~_upt~} {~_ch~} '_chid' \\?) \\)}            '_upc' ><
{_ch@ 0 \\( {~_lot~} {~_ch~} '_chid' \\?) \\)}            '_loc' ><
{1 \\( chars {_upc} map concat \\)}                      'upper' ><   {text \u2192 TEXT} //
{1 \\( chars {_loc} map concat \\)}                      'lower' ><   {TEXT \u2192 text} //

{0 \\( _fp _N \\#l {~_FI~} {~_N~} \\<< \\) _format1..?} _format$
{0 \\( {} _y\` _fp {~_FI~} _x \\[l] _fa {~_FI~} _y \\[l] _FI; \\) {~_x~} {~_y~} _format..} _format1$
{_f@ _fa gather 0 \\( {~_f~} _fp {%} \\/l 0 _FI# \\) _format.. concat}       'format' ><  {x\u2026 text \u2192 text with each % filled in order} //
{format \\>_}                                           'printf' ><  {x\u2026 text \u2192 : print it} //
{glue {~_nl~} cat \\>_}                                 'say' ><     {x\u2026 \u2192 : print the line, then a newline} //

{records: name/key is an ordinary variable, name/keys lists the keys in the order they were first set} //
{_dk@ _dn@ {~_lb~~~~_dn~/~_dk~~~~_rb~} \\~}                                      'get' ><   {name key \u2192 value, 404 when unset} //
{_dv@ _dk@ _dn@ 0 \\( {~_dn~} {~_dk~} get {404} \\= \\) 0 \\( {~_dk~} {~_dn~/keys} \\l? \\// \\) 0 \\( {~_dv~} {~_dn~/~_dk~\`} \\~ \\)}   'set' ><  {name key value \u2192 : store} //
{_dn@ {~_dn~/keys} spill}                                                       'keys' ><  {name \u2192 key\u2026} //

\\)
`};window.S={Rt:M,LIB:et,CMD_WORDS:tt};})();
