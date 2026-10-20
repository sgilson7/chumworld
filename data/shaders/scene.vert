#version 300 es
// One shape: the mesh vertex placed by the instance's three columns and
// translation (iA, iB, iC; translation in .w), coloured by iD (rgb, and in
// .a the pass's own value). Every value arrives as the core's Q16 integer
// and is scaled by uQ. No number lives here: the pass flags, the biases and
// the scale are uniforms from the core (crates/core/src/look.rs, render.rs).
in vec3 aPos;
in vec3 aNormal;
in vec4 iA;
in vec4 iB;
in vec4 iC;
in vec4 iD;
uniform mat4 uVP;
uniform float uQ;
// the pass, as flags of 0 and 1: sky, ink, glow, dither; then shade
uniform vec4 uModeA;
uniform vec4 uModeB;
// the ink hull pushed back, halos and shadows pulled forward, as share of
// clip w, times the core's ONE (so scaled by uQ here): a tie in depth with a
// solid then always goes the solid's way, or the halo's (crates/core/src/flicker.rs)
uniform vec2 uBias;
out vec3 vWorld;
out vec3 vNormal;
out vec4 vColor;
out vec3 vLocal;
void main() {
  vec3 a = iA.xyz * uQ;
  vec3 b = iB.xyz * uQ;
  vec3 c = iC.xyz * uQ;
  vec3 t = vec3(iA.w, iB.w, iC.w) * uQ;
  vec3 p = aPos * uQ;
  vec3 n = aNormal * uQ;
  vec3 w = a * p.x + b * p.y + c * p.z + t;
  // the columns carry a scale that differs by axis, so normals take the
  // inverse scale: each axis divided by its column's squared length
  vNormal = a * (n.x / dot(a, a)) + b * (n.y / dot(b, b)) + c * (n.z / dot(c, c));
  vWorld = w;
  vColor = iD * uQ;
  vLocal = p;
  gl_Position = uVP * vec4(w, 1.0);
  gl_Position.z += (uModeA.y * uBias.x - (uModeA.z + uModeB.x) * uBias.y) * uQ * gl_Position.w;
  // the sky dome: never clipped by the near or the far plane
  if (uModeA.x > 0.0) gl_Position.z = 0.0;
}
