use bevy::{asset::embedded_asset,
           core_pipeline::{FullscreenShader,
                           schedule::{Core3d, Core3dSystems},
                           tonemapping::tonemapping},
           ecs::query::QueryItem,
           post_process::bloom::bloom,
           prelude::*,
           render::{Render, RenderApp, RenderStartup, RenderSystems,
                    extract_component::{ComponentUniforms, DynamicUniformIndex,
                                        ExtractComponent, ExtractComponentPlugin,
                                        UniformComponentPlugin},
                    render_resource::{BindGroupEntries, BindGroupLayoutDescriptor,
                                      BindGroupLayoutEntries, CachedRenderPipelineId,
                                      ColorTargetState, ColorWrites, FilterMode,
                                      FragmentState, Operations, PipelineCache,
                                      RenderPassColorAttachment, RenderPassDescriptor,
                                      RenderPipelineDescriptor, Sampler,
                                      SamplerBindingType, SamplerDescriptor,
                                      ShaderStages, ShaderType,
                                      SpecializedRenderPipeline,
                                      SpecializedRenderPipelines, TextureFormat,
                                      TextureSampleType, TextureUsages,
                                      binding_types::{sampler, texture_2d,
                                                      texture_depth_2d,
                                                      uniform_buffer}},
                    renderer::{RenderContext, RenderDevice, ViewQuery},
                    sync_component::SyncComponent,
                    view::{ExtractedView, ViewDepthTexture, ViewTarget}},
           shader::Shader};

#[derive(Component, Clone, Copy)]
pub struct DistanceBlur {
  pub from: f32,
  pub to: f32,
  pub spread: f32
}

#[derive(Component, ShaderType, Clone, Copy)]
pub struct Blurring {
  near: f32,
  onset: f32,
  to: f32,
  spread: f32
}

#[derive(Component)]
pub struct BlurPipelineId(CachedRenderPipelineId);

impl SyncComponent for DistanceBlur {
  type Target = (Blurring, BlurPipelineId);
}

impl ExtractComponent for DistanceBlur {
  type QueryData = (&'static DistanceBlur, &'static Projection);
  type QueryFilter = ();
  type Out = Blurring;

  fn extract_component(
    (&DistanceBlur { from, to, spread }, projection): QueryItem<'_, '_, Self::QueryData>
  ) -> Option<Blurring> {
    match projection {
      &Projection::Perspective(PerspectiveProjection { near, .. }) => {
        Some(Blurring { near, onset: from, to, spread })
      }
      _ => None
    }
  }
}

#[derive(Resource)]
struct BlurPipeline {
  layout: BindGroupLayoutDescriptor,
  sampler: Sampler,
  shader: Handle<Shader>,
  fullscreen: FullscreenShader
}

impl SpecializedRenderPipeline for BlurPipeline {
  type Key = TextureFormat;

  fn specialize(&self, format: TextureFormat) -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
      label: Some("distance blur".into()),
      layout: vec![self.layout.clone()],
      vertex: self.fullscreen.to_vertex_state(),
      fragment: Some(FragmentState {
        shader: self.shader.clone(),
        targets: vec![Some(ColorTargetState {
          format,
          blend: None,
          write_mask: ColorWrites::ALL
        })],
        ..default()
      }),
      ..default()
    }
  }
}

fn init_pipeline(
  mut commands: Commands,
  device: Res<RenderDevice>,
  assets: Res<AssetServer>,
  fullscreen: Res<FullscreenShader>
) {
  commands.insert_resource(BlurPipeline {
    layout: BindGroupLayoutDescriptor::new(
      "distance blur",
      &BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
          texture_2d(TextureSampleType::Float { filterable: true }),
          texture_depth_2d(),
          sampler(SamplerBindingType::Filtering),
          uniform_buffer::<Blurring>(true)
        )
      )
    ),
    sampler: device.create_sampler(&SamplerDescriptor {
      mag_filter: FilterMode::Linear,
      min_filter: FilterMode::Linear,
      ..default()
    }),
    shader: assets.load("embedded://skyrim2/blur.wgsl"),
    fullscreen: fullscreen.clone()
  })
}

fn expose_depth(mut cameras: Query<&mut Camera3d, With<Blurring>>) {
  cameras.iter_mut().for_each(|mut camera| {
    let usages = TextureUsages::from(camera.depth_texture_usages);
    if !usages.contains(TextureUsages::TEXTURE_BINDING) {
      camera.depth_texture_usages = (usages | TextureUsages::TEXTURE_BINDING).into();
    }
  })
}

fn prepare_pipelines(
  mut commands: Commands,
  cache: Res<PipelineCache>,
  pipeline: Res<BlurPipeline>,
  mut pipelines: ResMut<SpecializedRenderPipelines<BlurPipeline>>,
  views: Query<(Entity, &ExtractedView), With<Blurring>>
) {
  views.iter().for_each(|(entity, view)| {
    commands.entity(entity).insert(BlurPipelineId(pipelines.specialize(
      &cache,
      &pipeline,
      view.target_format
    )));
  })
}

fn blur(
  view: ViewQuery<(
    &ViewTarget,
    &ViewDepthTexture,
    &DynamicUniformIndex<Blurring>,
    &BlurPipelineId
  )>,
  cache: Res<PipelineCache>,
  pipeline: Res<BlurPipeline>,
  uniforms: Res<ComponentUniforms<Blurring>>,
  mut ctx: RenderContext
) {
  let (target, depth, index, &BlurPipelineId(id)) = view.into_inner();
  if let Some(render_pipeline) = cache.get_render_pipeline(id)
    && let Some(uniforms) = uniforms.uniforms().binding()
  {
    let post = target.post_process_write();
    let bind_group = ctx.render_device().create_bind_group(
      "distance blur",
      &cache.get_bind_group_layout(&pipeline.layout),
      &BindGroupEntries::sequential((
        post.source,
        depth.view(),
        &pipeline.sampler,
        uniforms
      ))
    );
    let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
      label: Some("distance blur"),
      color_attachments: &[Some(RenderPassColorAttachment {
        view: post.destination,
        depth_slice: None,
        resolve_target: None,
        ops: Operations::default()
      })],
      depth_stencil_attachment: None,
      timestamp_writes: None,
      occlusion_query_set: None,
      multiview_mask: None
    });
    pass.set_pipeline(render_pipeline);
    pass.set_bind_group(0, &bind_group, &[index.index()]);
    pass.draw(0..3, 0..1);
  }
}

pub fn plugin(app: &mut App) {
  embedded_asset!(app, "blur.wgsl");
  app.add_plugins((
    ExtractComponentPlugin::<DistanceBlur>::default(),
    UniformComponentPlugin::<Blurring>::default()
  ));
  app
    .sub_app_mut(RenderApp)
    .init_resource::<SpecializedRenderPipelines<BlurPipeline>>()
    .add_systems(RenderStartup, init_pipeline)
    .add_systems(
      Render,
      (
        expose_depth.in_set(RenderSystems::PrepareViews),
        prepare_pipelines.in_set(RenderSystems::Prepare)
      )
    )
    .add_systems(
      Core3d,
      blur.after(bloom).before(tonemapping).in_set(Core3dSystems::PostProcess)
    );
}
