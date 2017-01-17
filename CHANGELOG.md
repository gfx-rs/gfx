## Change Log

### Version 0.9 (2016-01-22)
  - Pipepeline State Object revolution (#828)
- v0.9.1 (2016-02-19)
  - window resize support (#879)
  - deriving windows attributes from target formats (#874)
- v0.9.2 (2016-02-24)
  - fixed universal format views (#886)
  - fixed constant buffer binding (#828)

### Version 0.10 (2016-03-21)
  - Direct3D 11 backend (#861)
- v0.10.1 (2016-03-26)
  - fixed update_texture (#912)
- v0.10.2 (2016-04-15)
  - fixed get_texel_count (#937)

### Version 0.11 (2016-04-30)
  - modified `Slice` API (#955)
  - fixed GL blending where it's not in the core (#953)
  - raw PSO components for vertex buffers and render targets

### Version 0.12 (2016-06-23)
  - Android / GLES support (#993)
  - GL unsigned int samplers (#991)
  - better errors (#976)
  - better GLSL pre core reflection

### Version 0.13 (2016-12-18)
  - experimental Metal backend (#969, #1049, #1050)
  - persistent mapping (#1026)
  - tessellation support (#1027, #1088)
  - new examples: gamma, particle, terrain_tessellated
  - better PSO error messages, constant offset checks (#1004)
  - unified scissor: now Y-reversed on GL (#1092)
  - "const" resources are now called "immutable"
  - faster handle clones and cleaner core  API (#1031)

### Version 0.14 (2017-01-16)
  - fixed `Fence` and `Sync` bounds (#1095) 
  - dx11 buffer mapping support (#1099, #1105)
  - redesigned resource usage model for next-gen compatibility (#1123)
  - buffer copy support (#1129)
  - fixed and improved some errors (#1137, #1138)
  - application launcher revamp and resize support (#1121)
