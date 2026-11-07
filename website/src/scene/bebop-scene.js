import * as THREE from "three";
import { simplex3d, curl3 } from "math/noise";
import { mulberry32 } from "math/random";

(() => {
  const mount = document.getElementById("bbStage");
  const hero = document.querySelector(".bb-hero");
  const viewportMode = mount.dataset.viewport === "true";
  const emptySpace = mount.dataset.empty === "true";
  if (!mount || !hero) return;

  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: false,
      powerPreference: "high-performance",
    });
  } catch (error) {
    mount.hidden = true;
    return;
  }
  if (!renderer) {
    mount.hidden = true;
    return;
  }

  const DPR = Math.min(window.devicePixelRatio || 1, 1.5);
  renderer.setPixelRatio(DPR);
  renderer.setClearColor(0x05070d, 1);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.autoClear = false;
  renderer.domElement.setAttribute("aria-hidden", "true");
  mount.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 100);
  camera.position.set(0, 0, 7);
  camera.lookAt(0, 0, 0);
  scene.add(new THREE.AmbientLight(0x223344, 2.2));
  const keyLight = new THREE.DirectionalLight(0xffd7a3, 2.8);
  keyLight.position.set(5, 4, 6);
  scene.add(keyLight);

  const bgScene = new THREE.Scene();
  const bgCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  const bgUniforms = {
    uRes: { value: new THREE.Vector2(1, 1) },
    uTime: { value: 0 },
  };
  const bgMaterial = new THREE.ShaderMaterial({
    uniforms: bgUniforms,
    depthTest: false,
    depthWrite: false,
    vertexShader: `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = vec4(position.xy, 0.0, 1.0);
      }
    `,
    fragmentShader: `
      precision highp float;
      uniform vec2 uRes;
      uniform float uTime;

      float hash21(vec2 p) {
        p = fract(p * vec2(123.34, 456.21));
        p += dot(p, p + 45.32);
        return fract(p.x * p.y);
      }
      float noise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        f = f * f * (3.0 - 2.0 * f);
        float a = hash21(i);
        float b = hash21(i + vec2(1.0, 0.0));
        float c = hash21(i + vec2(0.0, 1.0));
        float d = hash21(i + vec2(1.0, 1.0));
        return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
      }
      float fbm(vec2 p) {
        float v = 0.0;
        float a = 0.5;
        mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
        for (int i = 0; i < 5; i++) {
          v += a * noise(p);
          p = rot * p * 2.02 + 11.7;
          a *= 0.5;
        }
        return v;
      }
      float stars(vec2 p, float scale, float seed, float t) {
        vec2 gp = p * scale;
        vec2 id = floor(gp);
        vec2 f = fract(gp) - 0.5;
        float h = hash21(id + seed);
        float keep = step(0.935, h);
        vec2 off = (vec2(hash21(id + seed + 1.7), hash21(id + seed + 3.1)) - 0.5) * 0.6;
        float d = length(f - off);
        float star = (1.0 - smoothstep(0.0, 0.06, d)) * keep;
        return star * (0.5 + 0.5 * sin(uTime * 1.1 + h * 31.0));
      }

      void main() {
        vec2 p = (gl_FragCoord.xy - 0.5 * uRes) / uRes.y;
        float t = 14.0;

        vec3 col = mix(vec3(0.008, 0.016, 0.026), vec3(0.016, 0.046, 0.068), smoothstep(-0.55, 0.55, p.y));

        vec2 q = p + vec2(0.06, 0.01);
        q.x += 0.06 * sin(t * 0.7);
        q.y += 0.04 * cos(t * 0.5);
        vec2 w = vec2(
          fbm(q * 1.25 + vec2(0.0, t * 0.16)),
          fbm(q * 1.25 + vec2(5.2, -t * 0.13))
        );
        float n = fbm(q * 2.0 + 1.65 * w + vec2(t * 0.2, -t * 0.1));
        float n2 = fbm(q * 3.2 - 1.1 * w + vec2(-t * 0.12, t * 0.16));
        float mask = 1.0 - smoothstep(0.05, 1.5, length(p - vec2(0.53, 0.02)));
        float warm = smoothstep(0.3, 0.78, fbm(q * 1.55 + vec2(4.1, t * 0.1)));

        vec3 teal = vec3(0.025, 0.18, 0.23);
        vec3 cyan = vec3(0.08, 0.42, 0.50);
        vec3 violet = vec3(0.12, 0.045, 0.15);
        vec3 amber = vec3(0.48, 0.16, 0.055);
        vec3 rust = vec3(0.22, 0.06, 0.025);
        vec3 nebula = mix(teal, cyan, smoothstep(0.36, 0.7, n));
        nebula = mix(nebula, mix(amber, rust, warm), 0.48);
        nebula = mix(nebula, violet, smoothstep(0.56, 0.86, n2) * 0.32);

        float neb = smoothstep(0.4, 0.75, n) * mask;
        float filament = smoothstep(0.47, 0.8, n2) * mask;
        float violetFilament = smoothstep(0.6, 0.9, n2) * mask;
        col += nebula * neb * 0.84;
        col += mix(cyan, amber, warm) * filament * 0.24;
        col += mix(cyan, violet, 0.45) * violetFilament * 0.13;

        float milky = exp(-pow((p.y + p.x * 0.55 - 0.02) * 3.8, 2.0));
        col += vec3(0.08, 0.13, 0.17) * milky * 0.11;

        float ribbon = smoothstep(0.5, 0.86, fbm(vec2(q.x * 1.15 + w.x * 0.6, q.y * 3.6 - t * 0.18 + w.y * 0.55)));
        col += mix(cyan, amber, smoothstep(0.2, 0.8, warm)) * ribbon * smoothstep(-0.6, 0.2, p.x) * 0.24;

        float dust = smoothstep(0.55, 0.86, fbm(q * 2.6 + vec2(-2.0, t * 0.08)));
        col *= 1.0 - 0.25 * dust * mask;

        float star = 0.0;
        star += stars(p + vec2(0.0, t * 0.003), 34.0, 1.2, t) * 0.8;
        star += stars(p + vec2(2.4, -t * 0.002), 57.0, 7.4, t) * 0.5;
        star += stars(p + vec2(-3.1, t * 0.002), 89.0, 13.8, t) * 0.35;
        col += vec3(0.92, 0.96, 1.0) * star;

        vec2 starP = vec2(-0.42, 0.27);
        float starD = length(p - starP);
        float starCore = smoothstep(0.017, 0.0, starD);
        float starHalo = smoothstep(0.1, 0.0, starD) * 0.1;
        float spikeX = smoothstep(0.0035, 0.0, abs(p.y - starP.y)) * smoothstep(0.11, 0.0, abs(p.x - starP.x));
        float spikeY = smoothstep(0.0035, 0.0, abs(p.x - starP.x)) * smoothstep(0.11, 0.0, abs(p.y - starP.y));
        col += vec3(1.0, 0.82, 0.54) * starCore * 1.35;
        col += vec3(0.45, 0.65, 0.75) * starHalo;
        col += vec3(0.55, 0.75, 0.85) * (spikeX + spikeY) * 0.18;

        float vig = 1.0 - smoothstep(0.2, 1.4, length(p * vec2(0.82, 1.0)));
        col *= 0.72 + 0.28 * vig;
        float grain = hash21(gl_FragCoord.xy + fract(uTime) * 91.7);
        col += (grain - 0.5) * 0.012;
        gl_FragColor = vec4(max(col, 0.0), 1.0);
      }
    `,
  });
  const bgMesh = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), bgMaterial);
  bgScene.add(bgMesh);

  const planetGroup = new THREE.Group();
  scene.add(planetGroup);

  const planetUniforms = { uTime: { value: 0 } };
  const planetMaterial = new THREE.ShaderMaterial({
    uniforms: planetUniforms,
    vertexShader: `
      varying vec3 vObj;
      varying vec3 vNormal;
      varying vec3 vView;
      void main() {
        vObj = position;
        vec4 mv = modelViewMatrix * vec4(position, 1.0);
        vNormal = normalize(normalMatrix * normal);
        vView = -mv.xyz;
        gl_Position = projectionMatrix * mv;
      }
    `,
    fragmentShader: `
      precision highp float;
      varying vec3 vObj;
      varying vec3 vNormal;
      varying vec3 vView;
      uniform float uTime;

      float hash21(vec2 p) {
        p = fract(p * vec2(123.34, 456.21));
        p += dot(p, p + 45.32);
        return fract(p.x * p.y);
      }
      float noise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        f = f * f * (3.0 - 2.0 * f);
        float a = hash21(i);
        float b = hash21(i + vec2(1.0, 0.0));
        float c = hash21(i + vec2(0.0, 1.0));
        float d = hash21(i + vec2(1.0, 1.0));
        return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
      }
      float fbm(vec2 p) {
        float v = 0.0;
        float a = 0.5;
        mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
        for (int i = 0; i < 5; i++) {
          v += a * noise(p);
          p = rot * p * 2.02 + 11.7;
          a *= 0.5;
        }
        return v;
      }

      void main() {
        float lon = atan(vObj.x, vObj.z);
        float lat = asin(clamp(vObj.y, -1.0, 1.0));

        float flow = fbm(vec2(lon * 1.6, lat * 2.4));
        float band = sin(lat * 15.0 + flow * 4.2 + sin(lon * 2.2) * 1.0);
        float b = band * 0.5 + 0.5;

        vec3 deep = vec3(0.010, 0.036, 0.060);
        vec3 mid = vec3(0.030, 0.120, 0.165);
        vec3 warm = vec3(0.34, 0.13, 0.045);
        vec3 cream = vec3(0.62, 0.47, 0.28);

        vec3 surface = mix(deep, mid, smoothstep(0.08, 0.66, b));
        surface = mix(surface, warm, smoothstep(0.44, 0.80, b) * 0.68);
        surface = mix(surface, cream, smoothstep(0.76, 0.97, b) * 0.24);

        float shear = fbm(vec2(lon * 3.2, lat * 9.0) + flow * 1.8);
        surface *= 0.90 + shear * 0.20;

        float cloudNoise = fbm(vec2(lon * 3.8 + 2.2, lat * 8.4 + 1.0));
        float clouds = smoothstep(0.52, 0.72, cloudNoise);
        surface = mix(surface, vec3(0.78, 0.70, 0.55), clouds * 0.24);
        float wisps = smoothstep(0.58, 0.82, fbm(vec2(lon * 7.5, lat * 16.0) + 4.0));
        surface = mix(surface, vec3(0.84, 0.75, 0.59), wisps * 0.11);

        float polar = smoothstep(0.58, 0.96, abs(lat) / 1.5708);
        surface = mix(surface, vec3(0.07, 0.15, 0.19), polar * 0.28);

        float spotLon = lon + 0.55;
        float spotLat = lat - 0.18;
        float spot = smoothstep(1.0, 0.05, length(vec2(spotLon * cos(lat) * 1.7, spotLat * 7.5)));
        surface = mix(surface, vec3(0.52, 0.18, 0.065), spot * 0.72);
        surface += vec3(0.24, 0.08, 0.025) * smoothstep(0.75, 0.08, length(vec2(spotLon * 2.6, spotLat * 11.0))) * 0.22;

        vec3 n = normalize(vNormal);
        vec3 L = normalize(vec3(-0.62, 0.48, 0.62));
        float diff = max(dot(n, L), 0.0);
        float light = 0.04 + 0.96 * smoothstep(-0.02, 0.62, diff);
        surface *= light;

        surface += vec3(0.018, 0.048, 0.065) * (1.0 - smoothstep(-0.15, 0.35, diff));

        float rim = pow(1.0 - max(dot(n, normalize(vView)), 0.0), 3.0);
        surface += vec3(0.13, 0.42, 0.48) * rim * 0.38;

        surface += (hash21(gl_FragCoord.xy) - 0.5) * 0.005;

        gl_FragColor = vec4(surface, 1.0);
      }
    `,
  });
  const planet = new THREE.Mesh(new THREE.SphereGeometry(1, 192, 128), planetMaterial);
  planetGroup.add(planet);

  const atmosphere = new THREE.Mesh(
    new THREE.SphereGeometry(1.055, 96, 96),
    new THREE.ShaderMaterial({
      side: THREE.BackSide,
      transparent: true,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      uniforms: {},
      vertexShader: `
        varying vec3 vNormal;
        varying vec3 vView;
        void main() {
          vec4 mv = modelViewMatrix * vec4(position, 1.0);
          vNormal = normalize(normalMatrix * normal);
          vView = -mv.xyz;
          gl_Position = projectionMatrix * mv;
        }
      `,
      fragmentShader: `
        precision highp float;
        varying vec3 vNormal;
        varying vec3 vView;
        void main() {
          float rim = pow(1.0 - abs(dot(normalize(vNormal), normalize(vView))), 3.4);
          vec3 tint = mix(vec3(0.04, 0.30, 0.36), vec3(0.38, 0.15, 0.05), 0.16);
          gl_FragColor = vec4(tint * rim, rim * 0.68);
        }
      `,
    })
  );
  planetGroup.add(atmosphere);

  const haloMaterial = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    uniforms: {},
    vertexShader: `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: `
      precision highp float;
      varying vec2 vUv;
      void main() {
        vec2 q = vUv - 0.5;
        float d = length(q);
        float halo = exp(-d * 7.5) * 0.15;
        float inner = smoothstep(0.30, 0.14, d) * 0.04;
        vec3 tint = mix(vec3(0.05, 0.30, 0.36), vec3(0.34, 0.13, 0.05), smoothstep(0.16, 0.44, d));
        gl_FragColor = vec4(tint * (halo + inner), halo + inner);
      }
    `,
  });
  const halo = new THREE.Mesh(new THREE.PlaneGeometry(3.0, 3.0), haloMaterial);
  halo.position.z = -0.4;
  planetGroup.add(halo);

  const ringUniforms = { uTime: { value: 0 } };
  const ringMaterial = new THREE.ShaderMaterial({
    uniforms: ringUniforms,
    transparent: true,
    side: THREE.DoubleSide,
    depthWrite: false,
    vertexShader: `
      varying vec3 vPos;
      void main() {
        vPos = position;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: `
      precision highp float;
      varying vec3 vPos;
      uniform float uTime;
      float hash21(vec2 p) {
        p = fract(p * vec2(123.34, 456.21));
        p += dot(p, p + 45.32);
        return fract(p.x * p.y);
      }
      float noise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        f = f * f * (3.0 - 2.0 * f);
        return mix(mix(hash21(i), hash21(i + vec2(1.0, 0.0)), f.x),
                   mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), f.x), f.y);
      }
      void main() {
        float r = length(vPos.xy);
        float band = sin(r * 74.0) * 0.5 + 0.5;
        float fine = noise(vec2(r * 180.0, 3.7));
        float broad = sin(r * 18.0 + 1.4) * 0.5 + 0.5;

        float gaps = smoothstep(0.012, 0.0, abs(r - 1.39));
        gaps += smoothstep(0.008, 0.0, abs(r - 1.63)) * 0.65;
        gaps += smoothstep(0.006, 0.0, abs(r - 1.80)) * 0.55;

        float edge = smoothstep(1.16, 1.18, r) * (1.0 - smoothstep(1.91, 1.96, r));
        float alpha = edge * (0.095 + band * 0.080 + fine * 0.035 + broad * 0.030);
        alpha *= 1.0 - clamp(gaps, 0.0, 0.85);
        alpha += smoothstep(1.81, 1.87, r) * (1.0 - smoothstep(1.91, 1.96, r)) * 0.07;

        vec3 ringColor = mix(vec3(0.16, 0.22, 0.28), vec3(0.70, 0.64, 0.50), band);
        ringColor = mix(ringColor, vec3(0.35, 0.58, 0.64), smoothstep(0.58, 1.0, fine) * 0.35);
        ringColor = mix(ringColor, vec3(0.80, 0.70, 0.52), smoothstep(0.72, 1.0, broad) * 0.18);
        gl_FragColor = vec4(ringColor, alpha);
      }
    `,
  });
  const ring = new THREE.Mesh(new THREE.RingGeometry(1.36, 1.94, 256, 1), ringMaterial);
  ring.rotation.set(-Math.PI / 2 + 0.26, 0, 0.20);
  planetGroup.add(ring);

  const makeOrbit = (rx, ry, color, opacity) => {
    const points = [];
    for (let i = 0; i <= 180; i += 1) {
      const a = (i / 180) * Math.PI * 2;
      points.push(new THREE.Vector3(Math.cos(a) * rx, Math.sin(a) * ry, 0.02));
    }
    const geometry = new THREE.BufferGeometry().setFromPoints(points);
    const line = new THREE.Line(
      geometry,
      new THREE.LineDashedMaterial({
        color,
        transparent: true,
        opacity,
        dashSize: 0.07,
        gapSize: 0.05,
        depthWrite: false,
      })
    );
    line.computeLineDistances();
    return line;
  };
  const orbitA = makeOrbit(2.15, 0.72, 0x77e7ff, 0.36);
  const orbitB = makeOrbit(1.62, 0.53, 0xffc061, 0.18);
  planetGroup.add(orbitA, orbitB);

  const moonMaterial = new THREE.ShaderMaterial({
    uniforms: {},
    vertexShader: `
      varying vec3 vObj;
      varying vec3 vNormal;
      varying vec3 vView;
      void main() {
        vObj = position;
        vec4 mv = modelViewMatrix * vec4(position, 1.0);
        vNormal = normalize(normalMatrix * normal);
        vView = -mv.xyz;
        gl_Position = projectionMatrix * mv;
      }
    `,
    fragmentShader: `
      precision highp float;
      varying vec3 vObj;
      varying vec3 vNormal;
      varying vec3 vView;

      float hash21(vec2 p) {
        p = fract(p * vec2(123.34, 456.21));
        p += dot(p, p + 45.32);
        return fract(p.x * p.y);
      }
      float noise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        f = f * f * (3.0 - 2.0 * f);
        float a = hash21(i);
        float b = hash21(i + vec2(1.0, 0.0));
        float c = hash21(i + vec2(0.0, 1.0));
        float d = hash21(i + vec2(1.0, 1.0));
        return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
      }
      float fbm(vec2 p) {
        float v = 0.0;
        float a = 0.5;
        mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
        for (int i = 0; i < 4; i++) {
          v += a * noise(p);
          p = rot * p * 2.02 + 11.7;
          a *= 0.5;
        }
        return v;
      }
      vec3 hash33(vec3 p) {
        p = fract(p * vec3(0.1031, 0.1030, 0.0973));
        p += dot(p, p.yxz + 33.33);
        return fract((p.xxy + p.yxx) * p.zyx);
      }
      float craterField(vec3 p) {
        vec3 i = floor(p);
        vec3 f = fract(p);
        float d = 1.0;
        for (int x = -1; x <= 1; x++) {
          for (int y = -1; y <= 1; y++) {
            for (int z = -1; z <= 1; z++) {
              vec3 g = vec3(float(x), float(y), float(z));
              vec3 o = hash33(i + g);
              vec3 r = g + o - f;
              d = min(d, length(r));
            }
          }
        }
        return d;
      }

      void main() {
        float lon = atan(vObj.x, vObj.z);
        float lat = asin(clamp(vObj.y, -1.0, 1.0));
        float maria = fbm(vec2(lon * 2.1, lat * 2.1) + 3.0);
        float detail = fbm(vec2(lon * 17.0, lat * 17.0) + 8.0);

        vec3 highland = vec3(0.44, 0.44, 0.41);
        vec3 mare = vec3(0.16, 0.18, 0.20);
        vec3 base = mix(highland, mare, smoothstep(0.40, 0.66, maria));
        base *= 0.84 + detail * 0.25;

        float crater = craterField(normalize(vObj) * 7.0);
        base = mix(base, base * 0.48, smoothstep(0.52, 0.12, crater) * 0.55);
        base = mix(base, vec3(0.58, 0.58, 0.54), smoothstep(0.44, 0.52, crater) * (1.0 - smoothstep(0.52, 0.60, crater)) * 0.20);

        vec3 n = normalize(vNormal);
        vec3 L = normalize(vec3(-0.58, 0.45, 0.67));
        float diff = max(dot(n, L), 0.0);
        float light = 0.035 + 0.965 * smoothstep(-0.08, 0.58, diff);
        base *= light;
        base += vec3(0.02, 0.045, 0.055) * (1.0 - smoothstep(-0.15, 0.35, diff));

        float rim = pow(1.0 - max(dot(n, normalize(vView)), 0.0), 4.0);
        base += vec3(0.48, 0.60, 0.66) * rim * 0.20;
        base += (hash21(gl_FragCoord.xy) - 0.5) * 0.008;

        gl_FragColor = vec4(base, 1.0);
      }
    `,
  });
  const moon = new THREE.Mesh(new THREE.SphereGeometry(1, 96, 64), moonMaterial);
  scene.add(moon);

  const waypointMaterial = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    vertexShader: `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: `
      precision highp float;
      varying vec2 vUv;

      void main() {
        vec2 q = vUv - 0.5;
        float a = atan(q.y, q.x);

        float cardinal = 0.120 * pow(max(0.0, cos(4.0 * a)), 1.3);
        float diagonal = 0.055 * pow(max(0.0, -cos(4.0 * a)), 1.3);
        float starR = 0.040 + cardinal + diagonal;
        float star = smoothstep(0.004, 0.0, length(q) - starR);
        float diamond = smoothstep(0.052, 0.0, abs(q.x) + abs(q.y));
        float core = smoothstep(0.038, 0.0, length(q));

        vec2 orbit = vec2(q.x / 1.15, q.y / 0.72);
        float orbitRing = smoothstep(0.0035, 0.0, abs(length(orbit) - 0.29));
        orbitRing *= 0.75;
        vec2 satellitePos = vec2(cos(2.35) * 0.29 * 1.15, sin(2.35) * 0.29 * 0.72);
        float satellite = smoothstep(0.012, 0.0, length(q - satellitePos));

        float glow = exp(-length(q) * 8.0) * 0.11;

        vec3 cream = vec3(1.0, 0.94, 0.84);
        vec3 cyan = vec3(0.46, 0.90, 1.0);
        vec3 amber = vec3(1.0, 0.76, 0.38);
        vec3 color = mix(cream, cyan, orbitRing * 0.85 + satellite);
        color = mix(color, amber, core * 0.60 + diamond * 0.20);

        float alpha = max(max(star, diamond * 0.86), max(orbitRing * 0.70, satellite));
        gl_FragColor = vec4(color, alpha + core * 0.45 + glow);
      }
    `,
  });
  const waypoint = new THREE.Mesh(new THREE.PlaneGeometry(0.50, 0.50), waypointMaterial);
  scene.add(waypoint);

  const TRAIL_COUNT = 22;
  const trailPositions = new Float32Array(TRAIL_COUNT * 3);
  const trailGeometry = new THREE.BufferGeometry();
  trailGeometry.setAttribute("position", new THREE.BufferAttribute(trailPositions, 3));
  const waypointTrail = new THREE.Line(
    trailGeometry,
    new THREE.LineBasicMaterial({
      color: 0x77e7ff,
      transparent: true,
      opacity: 0.38,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    })
  );
  scene.add(waypointTrail);

  const cometUniforms = {
    uTime: { value: 0 },
    uAlpha: { value: 0 },
  };
  const cometMaterial = new THREE.ShaderMaterial({
    uniforms: cometUniforms,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    vertexShader: `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: `
      precision highp float;
      varying vec2 vUv;
      uniform float uAlpha;
      void main() {
        vec2 q = vUv;
        float y = q.y - 0.5;
        float head = exp(-length(vec2((q.x - 0.82) * 2.4, y)) * 26.0);
        float tail = exp(-abs(y) * 22.0);
        tail *= smoothstep(0.0, 0.22, q.x) * (1.0 - smoothstep(0.22, 0.82, q.x)) * 0.55;
        float glow = exp(-length(vec2((q.x - 0.82) * 1.1, y)) * 9.0) * 0.16;
        vec3 color = mix(vec3(1.0, 0.95, 0.86), vec3(0.46, 0.90, 1.0), smoothstep(0.1, 0.5, 1.0 - q.x));
        gl_FragColor = vec4(color, (head + tail + glow) * uAlpha);
      }
    `,
  });
  const comet = new THREE.Mesh(new THREE.PlaneGeometry(1.45, 0.34), cometMaterial);
  scene.add(comet);

  if (emptySpace) {
    planetGroup.visible = false;
    moon.visible = false;
    waypoint.visible = false;
    waypointTrail.visible = false;
    comet.visible = false;
  }

  const PARTICLE_COUNT = 950;
  const rng = mulberry32.create(0x6d2b79f5);
  const simplex = simplex3d.create(0x9e3779b9);
  const flowSample = (x, y, z) => simplex3d.sample(simplex, x, y, z);
  const velocity = [0, 0, 0];
  const bounds = { x0: -2.2, x1: 4.4, y0: -2.3, y1: 2.3, z0: -2.1, z1: 2.1 };

  const particlePositions = new Float32Array(PARTICLE_COUNT * 3);
  const particleColors = new Float32Array(PARTICLE_COUNT * 3);
  const particleSizes = new Float32Array(PARTICLE_COUNT);
  const particleAlphas = new Float32Array(PARTICLE_COUNT);
  const palette = [
    [1.0, 0.94, 0.84],
    [0.46, 0.90, 1.0],
    [1.0, 0.75, 0.36],
    [0.63, 0.78, 0.82],
  ];
  for (let i = 0; i < PARTICLE_COUNT; i += 1) {
    particlePositions[i * 3] = bounds.x0 + (bounds.x1 - bounds.x0) * mulberry32.sample(rng);
    particlePositions[i * 3 + 1] = bounds.y0 + (bounds.y1 - bounds.y0) * mulberry32.sample(rng);
    particlePositions[i * 3 + 2] = bounds.z0 + (bounds.z1 - bounds.z0) * mulberry32.sample(rng);
    const color = palette[Math.floor(mulberry32.sample(rng) * palette.length)];
    particleColors[i * 3] = color[0];
    particleColors[i * 3 + 1] = color[1];
    particleColors[i * 3 + 2] = color[2];
    particleSizes[i] = 0.030 + mulberry32.sample(rng) * 0.060;
    particleAlphas[i] = 0.22 + mulberry32.sample(rng) * 0.50;
  }
  const particleGeometry = new THREE.BufferGeometry();
  particleGeometry.setAttribute("position", new THREE.BufferAttribute(particlePositions, 3));
  particleGeometry.setAttribute("aColor", new THREE.BufferAttribute(particleColors, 3));
  particleGeometry.setAttribute("aSize", new THREE.BufferAttribute(particleSizes, 1));
  particleGeometry.setAttribute("aAlpha", new THREE.BufferAttribute(particleAlphas, 1));

  const particleMaterial = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    uniforms: {
      uPixelRatio: { value: DPR },
      uTime: { value: 0 },
    },
    vertexShader: `
      attribute vec3 aColor;
      attribute float aSize;
      attribute float aAlpha;
      uniform float uPixelRatio;
      uniform float uTime;
      varying vec3 vColor;
      varying float vAlpha;
      void main() {
        vec4 mv = modelViewMatrix * vec4(position, 1.0);
        vColor = aColor;
        vAlpha = aAlpha * (0.72 + 0.28 * sin(uTime * 1.15 + position.x * 2.4 + position.z * 1.7));
        gl_PointSize = clamp(aSize * (560.0 * uPixelRatio / -mv.z), 1.0, 9.0);
        gl_Position = projectionMatrix * mv;
      }
    `,
    fragmentShader: `
      precision highp float;
      varying vec3 vColor;
      varying float vAlpha;
      void main() {
        float d = length(gl_PointCoord - 0.5);
        float alpha = smoothstep(0.5, 0.0, d);
        gl_FragColor = vec4(vColor, alpha * vAlpha);
      }
    `,
  });
  const particles = new THREE.Points(particleGeometry, particleMaterial);
  scene.add(particles);

  const gridScene = new THREE.Scene();
  const gridCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  const gridUniforms = {
    uRes: { value: new THREE.Vector2(1, 1) },
    uCell: { value: new THREE.Vector2(8.241, 16.8) },
    uGrid: { value: new THREE.Vector2(0, 0) },
  };
  const gridMaterial = new THREE.ShaderMaterial({
    uniforms: gridUniforms,
    transparent: true,
    depthTest: false,
    depthWrite: false,
    vertexShader: `
      void main() {
        gl_Position = vec4(position.xy, 0.0, 1.0);
      }
    `,
    fragmentShader: `
      precision highp float;
      uniform vec2 uRes;
      uniform vec2 uCell;
      uniform vec2 uGrid;
      void main() {
        vec2 top = vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y);
        vec2 g = mod(top - uGrid, uCell);
        float line = (1.0 - step(1.0, g.x)) + (1.0 - step(1.0, g.y));
        gl_FragColor = vec4(vec3(0.46, 0.90, 1.0), clamp(line, 0.0, 1.0) * 0.055);
      }
    `,
  });
  gridScene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), gridMaterial));

  let planetRadius = 0.82;
  let planetCenter = new THREE.Vector3();
  let viewHeight = 4.82;

  const updateGrid = () => {
    const styles = getComputedStyle(document.documentElement);
    const cw = parseFloat(styles.getPropertyValue("--cell-w")) || 8.241;
    const ch = parseFloat(styles.getPropertyValue("--cell-h")) || 16.8;
    const ox = parseFloat(styles.getPropertyValue("--bb-grid-offset-x")) || 0;
    const oy = parseFloat(styles.getPropertyValue("--bb-grid-offset-y")) || 0;
    gridUniforms.uCell.value.set(cw * DPR, ch * DPR);
    gridUniforms.uGrid.value.set(ox * DPR, oy * DPR);
  };

  const resize = () => {
    const width = Math.max(1, mount.clientWidth);
    const height = Math.max(1, mount.clientHeight);
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();

    const aspect = width / height;
    const mobile = width <= 760;
    planetRadius = viewportMode ? 0.98 : (mobile ? 0.55 : 0.82);
    viewHeight = 2 * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2)) * camera.position.z;
    const viewWidth = viewHeight * aspect;
    const cx = viewportMode ? viewWidth * 0.13 : (viewWidth / 2 - planetRadius * 0.43);
    const cy = viewportMode ? -0.18 : (mobile ? -1.35 : -0.08);
    planetCenter.set(cx, cy, 0);
    planetGroup.position.copy(planetCenter);
    planetGroup.scale.setScalar(planetRadius);

    bgUniforms.uRes.value.set(renderer.domElement.width, renderer.domElement.height);
    gridUniforms.uRes.value.set(renderer.domElement.width, renderer.domElement.height);
    updateGrid();
  };

  window.addEventListener("resize", resize);
  document.addEventListener("bb-grid-update", updateGrid);
  if ("ResizeObserver" in window) {
    new ResizeObserver(resize).observe(mount);
  }

  const clock = new THREE.Clock();
  let elapsed = 0;
  let frame = 0;
  let visible = true;
  let paused = false;

  const updateParticles = (dt, time) => {
    const pos = particleGeometry.attributes.position.array;
    const step = Math.min(dt, 0.05);
    for (let i = 0; i < PARTICLE_COUNT; i += 1) {
      const ix = i * 3;
      const x = pos[ix];
      const y = pos[ix + 1];
      const z = pos[ix + 2];
      curl3(velocity, flowSample, x * 0.20, y * 0.20, z * 0.20 + time * 0.004);
      pos[ix] += (velocity[0] * 0.060 + 0.0015) * step;
      pos[ix + 1] += velocity[1] * 0.060 * step;
      pos[ix + 2] += velocity[2] * 0.040 * step;

      if (pos[ix] > bounds.x1) pos[ix] = bounds.x0;
      if (pos[ix] < bounds.x0) pos[ix] = bounds.x1;
      if (pos[ix + 1] > bounds.y1) pos[ix + 1] = bounds.y0;
      if (pos[ix + 1] < bounds.y0) pos[ix + 1] = bounds.y1;
      if (pos[ix + 2] > bounds.z1) pos[ix + 2] = bounds.z0;
      if (pos[ix + 2] < bounds.z0) pos[ix + 2] = bounds.z1;
    }
    particleGeometry.attributes.position.needsUpdate = true;
  };

  const placeWaypoint = (time) => {
    const angle = 2.58 + time * 0.010;
    const p = new THREE.Vector3(
      Math.cos(angle) * 2.15 * planetRadius,
      Math.sin(angle) * 0.72 * planetRadius,
      0.04
    ).add(planetCenter);
    waypoint.position.copy(p);
    waypoint.quaternion.copy(camera.quaternion);

    const trail = waypointTrail.geometry.attributes.position.array;
    for (let i = 0; i < TRAIL_COUNT; i += 1) {
      const a = angle - i * 0.018;
      trail[i * 3] = planetCenter.x + Math.cos(a) * 2.15 * planetRadius;
      trail[i * 3 + 1] = planetCenter.y + Math.sin(a) * 0.72 * planetRadius;
      trail[i * 3 + 2] = 0.03;
    }
    waypointTrail.geometry.attributes.position.needsUpdate = true;
  };

  const placeMoon = (time) => {
    const angle = 4.05 + time * 0.008;
    moon.position.set(
      planetCenter.x + Math.cos(angle) * planetRadius * 1.70,
      planetCenter.y + Math.sin(angle) * planetRadius * 0.55,
      0.4
    );
    moon.scale.setScalar(planetRadius * 0.17);
  };

  const placeComet = (time) => {
    const cycle = 150.0;
    const phase = (time % cycle) / cycle;
    const visible = phase < 0.56;
    comet.visible = visible;
    waypointTrail.visible = true;
    if (!visible) return;
    const p = Math.min(1, phase / 0.56);
    const x = THREE.MathUtils.lerp(-4.7, 3.2, p);
    const y = THREE.MathUtils.lerp(2.15, 0.25, p);
    comet.position.set(x, y, -1.1);
    comet.quaternion.copy(camera.quaternion);
    comet.rotateZ(Math.atan2(0.25 - 2.15, 3.2 + 4.7));
    cometUniforms.uAlpha.value =
      smoothstep01(phase / 0.08) * (1 - smoothstep01((phase - 0.46) / 0.10));
  };

  const smoothstep01 = (v) => {
    const x = Math.max(0, Math.min(1, v));
    return x * x * (3 - 2 * x);
  };

  const draw = (time) => {
    elapsed = time;
    bgUniforms.uTime.value = time;
    particleMaterial.uniforms.uTime.value = time;

    planetUniforms.uTime.value = 12.0;
    ringUniforms.uTime.value = 8.0;
    planet.rotation.set(-0.06, 0.35, 0);
    ring.rotation.z = 0.20;
    orbitA.rotation.z = 0;
    orbitB.rotation.z = 0;
    placeMoon(0.0);
    placeWaypoint(0.0);
    comet.visible = false;
    waypointTrail.visible = false;
    updateParticles(1 / 600, time * 0.02);

    camera.position.set(0, 0, 7);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();

    renderer.clear();
    renderer.render(bgScene, bgCamera);
    renderer.clearDepth();
    renderer.render(scene, camera);
    renderer.clearDepth();
    renderer.render(gridScene, gridCamera);
  };

  const loop = () => {
    if (!visible || paused) return;
    draw(elapsed + Math.min(clock.getDelta(), 0.05));
    frame = requestAnimationFrame(loop);
  };

  const start = () => {
    if (!visible || paused) return;
    cancelAnimationFrame(frame);
    clock.start();
    frame = requestAnimationFrame(loop);
  };

  const stop = () => {
    cancelAnimationFrame(frame);
    clock.stop();
  };

  const activate = () => {
    resize();
    renderer.compile(scene, camera);
    draw(0);
    hero.classList.add("is-scene");
    start();
  };

  document.addEventListener("visibilitychange", () => {
    paused = document.hidden;
    if (paused) stop();
    else start();
  });

  canvasContextLoss();

  function canvasContextLoss() {
    renderer.domElement.addEventListener("webglcontextlost", (event) => {
      event.preventDefault();
      stop();
      mount.hidden = true;
      hero.classList.remove("is-scene");
    });
  }

  activate();
})();
