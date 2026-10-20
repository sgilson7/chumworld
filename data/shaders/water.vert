#version 300 es
// The river and the pond (M2, crates/core/src/water.rs), after Extremely
// Lifelike Fishing's water.vert: the core's still surface, displaced by the
// Gerstner sum the core steps once a tick (uPhase), each wave fading out
// where the eye could not tell it from the mesh's spacing, and calming in
// the shallows. The fishing game reads its flow map from a texture; here
// each vertex carries it: in aNormal the current along x and z (m/s) and the
// white water, in aColor the roughness and the depth to the ground (m, below
// zero under a bank). Four waves, as columns of two matrices, picked out by
// unit vectors: a shader's array size would be a number of its own (D3). No
// number lives here: every constant is a uniform from the core.
in vec3 aPos;
in vec3 aNormal;
in vec3 aColor;
in vec4 iA;
in vec4 iB;
in vec4 iC;
in vec4 iD;
uniform mat4 uVP;
uniform vec3 uEye;
// where positions are relative to, so the waves keep the world's phase
uniform vec2 uOrigin;
// each column a wave: direction x, z, turns a metre; height, sway, slope
uniform mat4 uWaveA;
uniform mat4 uWaveB;
// the waves' phases, in turns
uniform vec4 uPhase;
uniform float uTau;
// the mesh's spacing (m), metres more of it every metre away, the share of
// that spacing a wave fades from and to
uniform vec4 uLod;
// opacity a metre, at the edge, at the most; one over the depth the waves calm at
uniform vec4 uSee;
out vec3 vW;
out vec3 vStill;
out vec3 vN;
out float vFold;
out float vH;
out vec2 vFlow;
out float vWhite;
out float vRough;
out float vDepth;
out float vFade;

void wave(vec4 a, vec4 b, float phase, vec2 xz, float keep, inout vec3 p, inout vec3 n, inout float fold) {
  float spacing = uLod.x + length(p - uEye) * uLod.y;
  float fade = clamp((1.0 / (a.z * spacing) - uLod.z) / (uLod.w - uLod.z), 0.0, 1.0);
  vec4 c = b * keep * fade;
  float th = uTau * (dot(a.xy, xz) * a.z - phase);
  float s = sin(th);
  float co = cos(th);
  p.x += c.y * a.x * co;
  p.z += c.y * a.y * co;
  p.y += c.x * s;
  n.x -= a.x * c.z * co;
  n.z -= a.y * c.z * co;
  float q = c.y * uTau * a.z * s;
  n.y -= q;
  fold += q;
}

void main() {
  vec3 t = vec3(iA.w, iB.w, iC.w);
  vec3 w = iA.xyz * aPos.x + iB.xyz * aPos.y + iC.xyz * aPos.z + t;
  vStill = w;
  vec2 xz = w.xz + uOrigin;
  float rough = aColor.x;
  float depth = aColor.y;
  float keep = rough * clamp(depth * uSee.w, 0.0, 1.0);
  vec3 p = w;
  vec3 n = vec3(0.0, 1.0, 0.0);
  float fold = 0.0;
  vec4 e0 = vec4(1.0, 0.0, 0.0, 0.0);
  vec4 e1 = vec4(0.0, 1.0, 0.0, 0.0);
  vec4 e2 = vec4(0.0, 0.0, 1.0, 0.0);
  vec4 e3 = vec4(0.0, 0.0, 0.0, 1.0);
  wave(uWaveA * e0, uWaveB * e0, dot(uPhase, e0), xz, keep, p, n, fold);
  wave(uWaveA * e1, uWaveB * e1, dot(uPhase, e1), xz, keep, p, n, fold);
  wave(uWaveA * e2, uWaveB * e2, dot(uPhase, e2), xz, keep, p, n, fold);
  wave(uWaveA * e3, uWaveB * e3, dot(uPhase, e3), xz, keep, p, n, fold);
  vW = p;
  vN = n;
  vFold = fold;
  vH = p.y - w.y;
  vFlow = aNormal.xy;
  vWhite = aNormal.z;
  vRough = rough;
  vDepth = depth;
  vFade = iD.a;
  gl_Position = uVP * vec4(p - uEye, 1.0);
}
