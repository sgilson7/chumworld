#version 300 es
precision highp float;
// The river's and the pond's look (M2), after Extremely Lifelike Fishing's
// water.frag: the sky reflected by Fresnel, light through thin crests, sun
// glints, foam on folding crests, ripples carried by the current in two
// layers that fade as each resets (Vlachos's flow maps), streaks along the
// current, white water where the core's surface says (round the bridge's
// piers, the islet, the riffles), foam lapping where the water is shallow,
// and the bed showing through, more where the water is shallow. The sky and
// the haze are the scene's (scene.frag), so the water meets the land in the
// same light. The noise is periodic (uDrift.w cells) and the core sends its
// drift wrapped at that period, so neither the core nor this shader keeps a
// growing time. Each pixel is kept or dropped by the ordered dither (the
// hand-over to the far strip), and the far layer draws no water nearer than
// the near layer reaches (uCut), so no pixel is blended twice. No number
// lives here: every constant is a uniform from the core.
in vec3 vW;
in vec3 vStill;
in vec3 vN;
in float vFold;
in float vH;
in vec2 vFlow;
in float vWhite;
in float vRough;
in float vDepth;
in float vFade;
uniform vec3 uEye;
uniform vec2 uOrigin;
uniform float uCut;
uniform float uQ;
uniform vec3 uLightDir;
uniform vec3 uSun;
uniform vec3 uHorizon;
uniform vec3 uZenith;
uniform vec3 uSunGlow;
uniform vec4 uHazeShape;
// M2 (sky.rs): where the haze is made whole (m, m), the tint's share; the
// tint; where it rises, is whole, falls, is gone (m)
uniform vec4 uHazeEdge;
uniform vec3 uAerial;
uniform vec4 uAerialK;
uniform vec4 uSky;
uniform vec4 uK;
uniform mat4 uBayer;
uniform float uBayerSize;
uniform vec3 uDeep;
uniform vec3 uShallow;
uniform vec3 uCrest;
uniform vec3 uFoamColour;
// unused, Fresnel's base reflectance, Fresnel's power, crest light's power
uniform vec4 uLook;
// sharp glint power, sharp glint strength, soft glint power, soft glint strength
uniform vec4 uGlint;
// crest colour base, crest colour gain, sunlit base, sunlit gain
uniform vec4 uBody;
// crest light strength, crest light height gain, crest light height base, unused
uniform vec4 uCrestLight;
// fold where foam starts, fold where it is whole, foam strength, foam noise scale
uniform vec4 uFoam;
// foam edge low, foam edge high, foam cover, noise base
uniform vec4 uFoamEdge;
// ripple scale, second ripple scale, ripple drift speed (the core's), ripple strength
uniform vec4 uDetail;
// T34 (R9.563): a ripple layer fades out as a pixel comes to cover from the
// first to the second share of its noise's cell, so no ripple is drawn finer
// than the pixels can hold (CW-18)
uniform vec2 uDetailPx;
// the current a full streak stands for (m/s), streak strength, white water strength, flow cycle (s)
uniform vec4 uFlowK;
// streak scale along the current, across it, noise where streaks start, where they are whole
uniform vec4 uStreak;
// hash constants (Hoskins's hash without sine)
uniform vec4 uHash;
// opacity a metre, at the edge, at the most; one over the calm depth
uniform vec4 uSee;
// shore foam from and to (m), its strength, the ripples' fade distance (m)
uniform vec4 uShore;
// the ripples' drift, the foam's, the flow cycle's share (0 to 1), the noise's period
uniform vec4 uDrift;
out vec4 outColor;

float hash(vec2 p) {
  p = mod(p, uDrift.w);
  vec3 q = fract(vec3(p.xyx) * uHash.x);
  q += dot(q, q.yzx + uHash.w);
  return fract((q.x + q.y) * q.z);
}
float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (vec2(1.0) + vec2(1.0) + vec2(1.0) - f - f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0)), u.x), u.y);
}
// the eye-detail ripples' slope at a point: two noises at two scales, each
// weighed by how much of it the pixel can hold (k)
vec2 ripple(vec2 xz, vec2 k) {
  vec2 p1 = xz * uDetail.x - vec2(uDrift.x);
  vec2 p2 = xz * uDetail.y + vec2(uDrift.x, -uDrift.x);
  float h = uK.x;
  return vec2(noise(p1) - noise(p1 + vec2(h, 0.0)), noise(p1) - noise(p1 + vec2(0.0, h))) * k.x
       + vec2(noise(p2) - noise(p2 + vec2(h, 0.0)), noise(p2) - noise(p2 + vec2(0.0, h))) * k.y;
}
// the scene's sky (scene.frag)
vec3 sky(vec3 dir) {
  float up = clamp(dir.y, 0.0, 1.0);
  vec3 c = mix(uHorizon, uZenith, pow(up, uSky.x));
  float s = max(dot(dir, uLightDir), 0.0);
  return c + uSunGlow * pow(s, uSky.w) * (1.0 - up);
}
// the scene's haze (scene.frag)
vec4 haze() {
  vec3 to = vW - uEye;
  float d = length(to);
  vec3 flat_dir = normalize(vec3(to.x, 0.0, to.z) + vec3(0.0, uQ, 0.0));
  float f = min(1.0 - exp(-max(d - uHazeShape.x, 0.0) * uHazeShape.y * uQ), uHazeShape.z);
  // M2 (sky.rs): whole before the far plane; tinted cool over the middle distance
  f = max(f, smoothstep(uHazeEdge.x, uHazeEdge.y, d));
  vec3 c = mix(uHorizon, sky(flat_dir), uHazeShape.w);
  float k = smoothstep(uAerialK.x, uAerialK.y, d) * (1.0 - smoothstep(uAerialK.z, uAerialK.w, d));
  c = mix(c, uAerial, k * uHazeEdge.z);
  return vec4(c, f);
}
bool keep(float f) {
  vec2 p = mod(floor(gl_FragCoord.xy), uBayerSize);
  float t = uBayer[int(p.x)][int(p.y)];
  return f >= 0.0 ? t < f : t >= f + 1.0;
}

void main() {
  float dist = length(vStill - uEye);
  // the ground a pixel covers here, along whichever way it is longer (m)
  float foot = max(fwidth(vStill.x), fwidth(vStill.z));
  vec2 held = vec2(1.0) - smoothstep(vec2(uDetailPx.x), vec2(uDetailPx.y), foot * uDetail.xy);
  if (dist < uCut || !keep(vFade)) discard;
  vec2 xz = vW.xz + uOrigin;
  float h = uK.x;
  // the ripples, carried by the current in two layers, faded as each resets
  float fade = clamp(1.0 - dist / uShore.w, 0.0, 1.0);
  vec2 flow = vFlow;
  float speed = length(flow);
  float t1 = uDrift.z;
  float t2 = fract(t1 + h);
  float w1 = 1.0 - abs(t1 - h) / h;
  vec2 q1 = xz - flow * t1 * uFlowK.w;
  vec2 q2 = xz - flow * t2 * uFlowK.w;
  vec2 g = (ripple(q1, held) * w1 + ripple(q2, held) * (1.0 - w1)) * uDetail.w * fade * (h + vRough);
  // streaks that run with the current, and the white water the core lays out
  vec2 along = flow / (speed + uHash.y);
  vec2 across = vec2(-along.y, along.x);
  float s1 = noise(vec2(dot(q1, along) * uStreak.x, dot(q1, across) * uStreak.y));
  float s2 = noise(vec2(dot(q2, along) * uStreak.x, dot(q2, across) * uStreak.y));
  float streak = smoothstep(uStreak.z, uStreak.w, s1 * w1 + s2 * (1.0 - w1));
  float foamAdd = streak * clamp(speed / uFlowK.x, 0.0, 1.0) * uFlowK.y * fade;
  float churn = noise(q1 * uFoam.w) * w1 + noise(q2 * uFoam.w) * (1.0 - w1);
  foamAdd += vWhite * churn * uFlowK.z;
  // foam lapping at the shallow edge
  foamAdd += (1.0 - smoothstep(uShore.x, uShore.y, vDepth)) * uShore.z * noise(xz * uFoam.w + vec2(uDrift.y));
  // the still surface's own slope (standing waves, pillows), from its facets
  vec3 ns = normalize(cross(dFdx(vStill), dFdy(vStill)));
  if (ns.y < 0.0) ns = -ns;
  vec3 N = normalize(normalize(vN) + ns - vec3(0.0, 1.0, 0.0) + vec3(-g.x, 0.0, -g.y));
  vec3 V = normalize(uEye - vW);
  float fres = uLook.y + (1.0 - uLook.y) * pow(1.0 - max(dot(N, V), 0.0), uLook.z);
  vec3 Rf = reflect(-V, N);
  Rf.y = abs(Rf.y);
  vec3 refl = sky(Rf);
  float crest = clamp(vH * uBody.y + uBody.x, 0.0, 1.0);
  vec3 body = mix(uDeep, uShallow, crest);
  float sss = pow(max(dot(V, -uLightDir) * h + h, 0.0), uLook.w) * clamp(vH * uCrestLight.y + uCrestLight.z, 0.0, 1.0);
  body += uCrest * sss * uCrestLight.x;
  body *= uBody.z + uBody.w * max(dot(N, uLightDir), 0.0);
  vec3 col = mix(body, refl, clamp(fres, 0.0, 1.0));
  vec3 H = normalize(uLightDir + V);
  float nh = max(dot(N, H), 0.0);
  col += uSun * (pow(nh, uGlint.x) * uGlint.y + pow(nh, uGlint.z) * uGlint.w);
  float foam = smoothstep(uFoam.x, uFoam.y, vFold) * uFoam.z + foamAdd;
  foam = clamp(foam * (uFoamEdge.w + noise(xz * uFoam.w + vec2(uDrift.y * h, 0.0))), 0.0, 1.0);
  float white = smoothstep(uFoamEdge.x, uFoamEdge.y, foam) * uFoamEdge.z;
  col = mix(col, uFoamColour, white);
  // the bed shows through: more where the water is shallow and faces the eye
  float body_alpha = clamp(uSee.y + max(vDepth, 0.0) * uSee.x, 0.0, uSee.z);
  float alpha = clamp(mix(body_alpha, 1.0, fres) + white, 0.0, 1.0);
  // none at all past the waterline, under the bank
  alpha *= clamp(vDepth / uShore.x, 0.0, 1.0);
  vec4 hz = haze();
  col = mix(col, hz.rgb, hz.a);
  alpha = mix(alpha, 1.0, hz.a);
  outColor = vec4(col, alpha);
}
