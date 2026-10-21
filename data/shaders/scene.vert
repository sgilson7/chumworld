#version 300 es
// One shape: the mesh vertex placed by the instance's three columns and
// translation (iA, iB, iC; translation in .w), coloured by iD (rgb, and in
// .a the pass's own value), in metres, as floats from the shim. No number
// lives here: the pass flags and the biases are uniforms from the core
// (crates/core/src/look.rs, render.rs).
in vec3 aPos;
in vec3 aNormal;
// the vertex's own colour: white on a primitive, a facet's colour on a world mesh
in vec3 aColor;
in vec4 iA;
in vec4 iB;
in vec4 iC;
in vec4 iD;
// the layer's projection times the camera's turn; positions arrive
// relative to the eye (camera-relative drawing, crates/core/src/render.rs)
uniform mat4 uVP;
uniform vec3 uEye;
uniform float uQ;
// the pass, as flags of 0 and 1: sky, ink, glow, dither; then shade
uniform vec4 uModeA;
uniform vec4 uModeB;
// the ink hull pushed back, halos and shadows pulled forward, as share of
// clip w, times the core's ONE (so scaled by uQ here): a tie in depth with a
// solid then always goes the solid's way, or the halo's (crates/core/src/flicker.rs)
uniform vec2 uBias;
// M7 (scatter.rs, R9.44): bands a dithered shape's triangles fade through
// by their own distance from the eye, metres: in from x to y, out from z to w
uniform mat4 uBands;
uniform mat4 uBands2;
// how deep a shape of the second matrix's bands (grass) sinks into the ground
// as it fades, metres: it sinks rather than dithering, so a far band of grass
// leaves no stipple to crawl as the view pans
uniform float uSink;
// the value that names the first band, the first band that sinks, the
// first after them that dithers again, and the first that sinks again
uniform vec4 uBandKey;
// M7 (lod::index_flat): a vertex whose normal is longer than this is from an
// indexed facet mesh, whose triangles take their last vertex's values flat
uniform vec3 uFlatKey;
// M7: the wind's waves (direction x, z; radians a metre; share), a column each field,
// its bend, sheen, shadow and trough strength, and its phases this frame
uniform mat4 uWindA;
uniform vec4 uWindK;
uniform vec4 uWindPhase;

out vec3 vWorld;
// the dither's fade for the whole triangle, taken from its last vertex
flat out float vFade;
// an indexed facet mesh's normal and colour, flat, and whether it is one
flat out vec3 vNormalF;
flat out vec3 vColorF;
flat out float vFlat;
// the ground's own facets, which the fragment shades as grass where green
flat out float vGround;
out vec3 vNormal;
out vec4 vColor;
out vec3 vLocal;
// A triangle's fade at `d` metres through band `k` (scatter::band_fade):
// coming in, the share of the way through the first two distances; going
// out, the share through the last two less one.
float band(float d, float k) {
  vec4 b = k < uBandKey.y ? uBands[int(k)] : uBands2[int(k - uBandKey.y)];
  float come = b.y > b.x ? clamp((d - b.x) / (b.y - b.x), 0.0, 1.0) : (d >= b.x ? 1.0 : 0.0);
  float go = b.w > b.z ? clamp((d - b.z) / (b.w - b.z), 0.0, 1.0) : (d >= b.z ? 1.0 : 0.0);
  return go > 0.0 ? go - 1.0 : come;
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

void main() {
  vec3 a = iA.xyz;
  vec3 b = iB.xyz;
  vec3 c = iC.xyz;
  vec3 t = vec3(iA.w, iB.w, iC.w);
  vec3 p = aPos;
  vec3 n = aNormal;
  vec3 w = a * p.x + b * p.y + c * p.z + t;
  // a value of uBandKey.x or more names a band (scatter::band_value);
  // below, the fade itself; the second matrix's bands sink
  float k = iD.a - uBandKey.x;
  float f = k >= 0.0 ? band(length((w - uEye).xz), k) : iD.a;
  float len = length(n);
  if (k >= uBandKey.y && (k < uBandKey.z || k >= uBandKey.w)) {
    w.y -= (1.0 - (f >= 0.0 ? f : -f)) * uSink;
    f = 1.0;
    // grass bends in the wind: its tip by its share, written in its normal's
    // length (scatter::Bake::bend_from), its foot not at all
    float bend = clamp(len - uFlatKey.z, 0.0, 1.0);
    w.xz += vec2(uWindA[0].x, uWindA[1].x) * (wind(w.xz) * bend * uWindK.x);
  }
  // the columns carry a scale that differs by axis, so normals take the
  // inverse scale: each axis divided by its column's squared length
  vNormal = a * (n.x / dot(a, a)) + b * (n.y / dot(b, b)) + c * (n.z / dot(c, c));
  vWorld = w;
  vColor = vec4(iD.rgb * aColor, iD.a);
  vLocal = p;
  vFade = f;
  vNormalF = vNormal;
  vColorF = vColor.rgb;
  vFlat = len > uFlatKey.x ? 1.0 : 0.0;
  vGround = len > uFlatKey.y ? 1.0 : 0.0;
  gl_Position = uVP * vec4(w - uEye, 1.0);
  gl_Position.z += (uModeA.y * uBias.x - (uModeA.z + uModeB.x) * uBias.y) * uQ * gl_Position.w;
  // the sky dome: never clipped by the near or the far plane
  if (uModeA.x > 0.0) gl_Position.z = 0.0;
}
