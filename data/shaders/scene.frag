#version 300 es
precision highp float;
// The passes (the core says which, as flags in uModeA and uModeB): the sky,
// coloured by direction; solids, lit as Protection of the Spheres lights
// them (light wrapped past the terminator with a gentle ramp, a sky fill
// above and a ground bounce below, no hard shadows); dithered solids, the
// same on the pixels the ordered dither keeps; ink, the outline, flat; soft
// shadows, blended; and halos, added. Far things fade into the colour of the
// sky behind them (Iron Vector's haze). No number lives here: every constant
// is a uniform from the core (crates/core/src/look.rs).
in vec3 vWorld;
in vec3 vNormal;
in vec4 vColor;
in vec3 vLocal;
flat in float vFade;
flat in vec3 vNormalF;
flat in vec3 vColorF;
flat in float vFlat;
flat in float vGround;
// M7: the wind's waves (direction x, z; radians a metre; share), a column each field,
// its bend, sheen, shadow and trough strength, and its phases this frame
uniform mat4 uWindA;
uniform vec4 uWindK;
uniform vec4 uWindPhase;
// the troughs between the grass's clumps: radians a metre of each of the two
// crossing waves, the grass's two keys; their directions; their phases at
// the origin; how soon a pattern finer than a pixel fades (troughs, gusts)
uniform vec4 uTrough;
uniform vec4 uTroughDir;
uniform vec2 uTroughPhase;
uniform vec4 uWindAA;
uniform vec3 uEye;
uniform vec4 uModeA;
uniform vec4 uModeB;
uniform float uQ;
uniform vec3 uLightDir;
uniform vec3 uSun;
uniform vec3 uSkyFill;
uniform vec3 uBounce;
uniform vec3 uHorizon;
uniform vec3 uZenith;
uniform vec3 uSunGlow;
// where the haze starts (m), how thick it is per metre (times ONE), the most
// it hides, how far the sun's glow tints it
uniform vec4 uHazeShape;
// M2 (sky.rs): where the haze is made whole (m, m), the tint's share; the
// tint; where it rises, is whole, falls, is gone (m)
uniform vec4 uHazeEdge;
uniform vec3 uAerial;
uniform vec4 uAerialK;
// light wrapped past the terminator, the ramp's start and end, how much of the ramp
uniform vec4 uWrap;
// the sky's curve, the sun disc's inner and outer cosines, the sun glow's power
uniform vec4 uSky;
// a half, the halo's power, how far ink fades into the haze, the shadow's strength
uniform vec4 uK;
// the ordered dither's thresholds, column x, row y, and its size
uniform mat4 uBayer;
uniform float uBayerSize;
out vec4 outColor;

// The sky's colour looking along dir: the horizon's colour rising to the
// zenith's, warmed toward the sun.
vec3 sky(vec3 dir) {
  float up = clamp(dir.y, 0.0, 1.0);
  vec3 c = mix(uHorizon, uZenith, pow(up, uSky.x));
  float s = max(dot(dir, uLightDir), 0.0);
  return c + uSunGlow * pow(s, uSky.w) * (1.0 - up);
}

// The haze's colour and share at this fragment: the sky at the horizon
// behind it.
vec4 haze() {
  vec3 to = vWorld - uEye;
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

// M7 (wind.rs, R9.54): the wind at p, as the core sums it (`wind::gust`):
// each wave's share of sin(k d.p - phase), the phase carrying the origin.
// uWindA's columns: the four waves' direction x, direction z, radians a
// metre, share
float wind(vec2 p) {
  vec4 along = uWindA * vec4(p, 0.0, 0.0);
  vec4 k = uWindA * vec4(0.0, 0.0, 1.0, 0.0);
  vec4 a = uWindA * vec4(0.0, 0.0, 0.0, 1.0);
  return dot(a, sin(along * k - uWindPhase));
}

// Whether the ordered dither keeps this pixel at fade f (look::keeps).
bool keep(float f) {
  vec2 p = mod(floor(gl_FragCoord.xy), uBayerSize);
  float t = uBayer[int(p.x)][int(p.y)];
  return f >= 0.0 ? t < f : t >= f + 1.0;
}

void main() {
  if (uModeA.x > 0.0) {
    vec3 dir = normalize(vWorld - uEye);
    vec3 c = sky(dir);
    float s = dot(dir, uLightDir);
    c = mix(c, uSun + uSunGlow, smoothstep(uSky.z, uSky.y, s));
    outColor = vec4(c, 1.0);
    return;
  }
  if ((uModeA.y + uModeA.w) > 0.0 && !keep(vFade)) discard;
  vec4 h = haze();
  // M7: and its triangle's own colour
  vec3 facet = vFlat > 0.0 ? vColorF : vColor.rgb;
  if (uModeA.y > 0.0) {
    outColor = vec4(mix(facet, h.rgb, h.a * uK.z), 1.0);
    return;
  }
  // M7: an indexed facet mesh takes its triangle's own normal
  vec3 n = normalize(vFlat > 0.0 ? vNormalF : vNormal);
  if (uModeA.z > 0.0) {
    vec3 v = normalize(uEye - vWorld);
    float a = pow(abs(dot(n, v)), uK.y) * vColor.a * (1.0 - h.a);
    outColor = vec4(vColor.rgb * a, 1.0);
    return;
  }
  if (uModeB.x > 0.0) {
    float r = length(vLocal.xz) / uK.x;
    float a = uK.w * vColor.a * clamp(1.0 - r * r, 0.0, 1.0) * (1.0 - h.a);
    outColor = vec4(vColor.rgb, a);
    return;
  }
  vec3 base = facet;
  // M7 (R9.54): the ground where grass grows (green enough) is never a flat
  // fill: darker troughs between the clumps, and the wind's gusts rolling
  // sheen and shadow across it; each fades before it is finer than a pixel
  if (vGround > 0.0) {
    float grass = smoothstep(uTrough.z, uTrough.w, facet.g - max(facet.r, facet.b));
    vec2 p = vWorld.xz;
    float t1 = dot(uTroughDir.xy, p) * uTrough.x + uTroughPhase.x;
    float t2 = dot(uTroughDir.zw, p) * uTrough.y + uTroughPhase.y;
    float fine = clamp(1.0 - (fwidth(t1) + fwidth(t2)) * uWindAA.x, 0.0, 1.0);
    float trough = max(-sin(t1) * sin(t2), 0.0) * fine;
    float coarse = clamp(1.0 - fwidth(dot(vec2(uWindA[0].x, uWindA[1].x), p) * (uWindA * vec4(0.0, 0.0, 1.0, 0.0)).x) * uWindAA.y, 0.0, 1.0);
    float gust = wind(p) * coarse;
    base *= 1.0 + grass * (max(gust, 0.0) * uWindK.y - max(-gust, 0.0) * uWindK.z - trough * uWindK.w);
  }
  float wrapped = clamp((dot(n, uLightDir) + uWrap.x) / (1.0 + uWrap.x), 0.0, 1.0);
  float key = mix(wrapped, smoothstep(uWrap.y, uWrap.z, wrapped), uWrap.w);
  vec3 fill = mix(uBounce, uSkyFill, n.y * uK.x + uK.x);
  vec3 lit = base * (fill + uSun * key);
  // the dither pass carries a fade in .a, a solid its glow
  float glow = uModeA.w > 0.0 ? 0.0 : vColor.a;
  lit = mix(lit, base, glow);
  outColor = vec4(mix(lit, h.rgb, h.a), 1.0);
}
