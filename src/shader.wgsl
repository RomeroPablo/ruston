  struct VsOut {
      @builtin(position) position: vec4<f32>,
      @location(0) local_pos: vec2<f32>,
  };

  @vertex
  fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
      let positions = array<vec2<f32>, 3>(
          vec2<f32>( 0.0,  0.7),
          vec2<f32>(-0.7, -0.7),
          vec2<f32>( 0.7, -0.7),
      );

      var out: VsOut;
      let pos = positions[vertex_index];
      out.position = vec4<f32>(pos, 0.0, 1.0);
      out.local_pos = pos;
      return out;
  }

  @fragment
  fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
      let local = in.local_pos;
      return vec4<f32>(local.x * 0.5 + 0.5, local.y * 0.5 + 0.5, 1.0, 1.0);
  }
