//! JavaScript bindings for Mustang animations via Boa
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.
//!
//! Provides a JavaScript API for controlling animations using the Boa engine (v0.21).

use super::{AnimatedProperty, Animation, AnimationConfig, AnimationEngine, EasingFunction};
use crate::effect::Effect;
use boa_engine::object::ObjectInitializer;
use boa_engine::{Context, JsResult, JsString, JsValue, NativeFunction, Source, js_string};
use std::time::Duration;

/// JavaScript animation runtime using Boa 0.21
pub struct JsAnimationRuntime {
    /// Boa JS context
    context: Context,
    /// Animation engine
    engine: AnimationEngine,
    /// Viewport dimensions
    viewport: (u32, u32),
}

impl JsAnimationRuntime {
    /// Create a new JS animation runtime
    pub fn new(viewport: (u32, u32)) -> Self {
        let mut context = Context::default();
        let mut engine = AnimationEngine::new();

        // Register Mustang animation API
        Self::register_api(&mut context, &mut engine);

        Self {
            context,
            engine,
            viewport,
        }
    }

    /// Register the Mustang animation API with the JS context
    fn register_api(context: &mut Context, engine: &mut AnimationEngine) {
        // Register mustang global object
        let mustang_obj = Self::create_mustang_object(context, engine);
        let _ = context.register_global_property(
            js_string!("mustang"),
            mustang_obj,
            Default::default(),
        );

        // Register console.log for debugging
        let _ = context.register_global_builtin_callable(
            JsString::from("print"),
            1,
            NativeFunction::from_fn_ptr(Self::js_print),
        );
    }

    /// Create the mustang global object with animation methods
    fn create_mustang_object(context: &mut Context, _engine: &mut AnimationEngine) -> JsValue {
        let obj = ObjectInitializer::new(context)
            .function(
                NativeFunction::from_fn_ptr(Self::js_create_animation),
                js_string!("animate"),
                1,
            )
            .function(
                NativeFunction::from_fn_ptr(Self::js_ease),
                js_string!("ease"),
                2,
            )
            .build();

        obj.into()
    }

    /// JS function: print(message) - for debugging
    fn js_print(_this: &JsValue, args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
        if let Some(msg) = args.get(0) {
            if let Some(s) = msg.as_string() {
                println!("[JS] {}", s.to_std_string_escaped());
            } else {
                println!("[JS] {:?}", msg);
            }
        }
        Ok(JsValue::undefined())
    }

    /// JS function: mustang.animate(config) - create and start an animation
    fn js_create_animation(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        // Get config object from first argument
        let config_obj = args.get(0).ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("animate() requires a config object")
        })?;

        // Extract selector
        let selector = Self::get_string_property(config_obj, "selector", context)?
            .unwrap_or_else(|| ".animated".to_string());

        // Extract duration (milliseconds)
        let duration_ms =
            Self::get_number_property(config_obj, "duration", context)?.unwrap_or(300.0);

        // Extract easing
        let easing_str = Self::get_string_property(config_obj, "easing", context)?
            .unwrap_or_else(|| "ease".to_string());
        let easing = EasingFunction::from_str(&easing_str).unwrap_or(EasingFunction::Ease);

        // Create animation config
        let config = AnimationConfig {
            selector,
            duration: Duration::from_millis(duration_ms as u64),
            delay: Duration::from_millis(
                Self::get_number_property(config_obj, "delay", context)?.unwrap_or(0.0) as u64,
            ),
            easing,
            iterations: Self::get_number_property(config_obj, "iterations", context)?
                .map(|n| if n < 0.0 { None } else { Some(n as u32) })
                .flatten()
                .or(Some(1)),
            alternate: Self::get_bool_property(config_obj, "alternate", context)?.unwrap_or(false),
            fill_forwards: Self::get_bool_property(config_obj, "fillForwards", context)?
                .unwrap_or(true),
        };

        // Extract properties to animate
        let properties = Self::extract_animated_properties(config_obj, context)?;

        // Create animation object to return
        let anim_obj = ObjectInitializer::new(context)
            .property(
                js_string!("selector"),
                JsString::from(config.selector.clone()),
                Default::default(),
            )
            .property(js_string!("duration"), duration_ms, Default::default())
            .property(
                js_string!("easing"),
                JsString::from(easing_str),
                Default::default(),
            )
            .property(
                js_string!("propertyCount"),
                properties.len() as i32,
                Default::default(),
            )
            .build();

        // Note: We can't actually start the animation here because we don't have
        // access to the AnimationEngine. The animation would need to be queued
        // and started on the next tick() call.

        Ok(anim_obj.into())
    }

    /// JS function: mustang.ease(name, t) - apply easing function
    fn js_ease(_this: &JsValue, args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
        let easing_name = args
            .get(0)
            .and_then(|v| v.as_string())
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|| "linear".to_string());

        let t = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0);

        let easing = EasingFunction::from_str(&easing_name).unwrap_or(EasingFunction::Linear);
        let result = easing.apply(t as f32);

        Ok(JsValue::new(result as f64))
    }

    /// Helper: Get string property from JS object
    fn get_string_property(
        obj: &JsValue,
        name: &str,
        context: &mut Context,
    ) -> JsResult<Option<String>> {
        if let Some(object) = obj.as_object() {
            let key = js_string!(name);
            if let Ok(value) = object.get(key, context) {
                if let Some(s) = value.as_string() {
                    return Ok(Some(s.to_std_string_escaped()));
                }
            }
        }
        Ok(None)
    }

    /// Helper: Get number property from JS object
    fn get_number_property(
        obj: &JsValue,
        name: &str,
        context: &mut Context,
    ) -> JsResult<Option<f64>> {
        if let Some(object) = obj.as_object() {
            let key = js_string!(name);
            if let Ok(value) = object.get(key, context) {
                if let Some(n) = value.as_number() {
                    return Ok(Some(n));
                }
            }
        }
        Ok(None)
    }

    /// Helper: Get bool property from JS object
    fn get_bool_property(
        obj: &JsValue,
        name: &str,
        context: &mut Context,
    ) -> JsResult<Option<bool>> {
        if let Some(object) = obj.as_object() {
            let key = js_string!(name);
            if let Ok(value) = object.get(key, context) {
                if value.is_boolean() {
                    return Ok(Some(value.as_boolean().unwrap_or(false)));
                }
            }
        }
        Ok(None)
    }

    /// Extract animated properties from config object
    fn extract_animated_properties(
        obj: &JsValue,
        context: &mut Context,
    ) -> JsResult<Vec<AnimatedProperty>> {
        let mut properties = Vec::new();

        // Get the "properties" array/object
        if let Some(object) = obj.as_object() {
            if let Ok(props_val) = object.get(js_string!("properties"), context) {
                // Handle blur property
                if let Ok(blur) = Self::get_object_property(&props_val, "blur", context) {
                    if let (Some(from), Some(to)) = (
                        Self::get_number_property(&blur, "from", context)?,
                        Self::get_number_property(&blur, "to", context)?,
                    ) {
                        properties.push(AnimatedProperty::Blur {
                            from: from as f32,
                            to: to as f32,
                        });
                    }
                }

                // Handle scale property
                if let Ok(scale) = Self::get_object_property(&props_val, "scale", context) {
                    if let (Some(from), Some(to)) = (
                        Self::get_number_property(&scale, "from", context)?,
                        Self::get_number_property(&scale, "to", context)?,
                    ) {
                        properties.push(AnimatedProperty::Scale {
                            from: from as f32,
                            to: to as f32,
                        });
                    }
                }

                // Handle translate property
                if let Ok(translate) = Self::get_object_property(&props_val, "translate", context) {
                    let from_x =
                        Self::get_number_property(&translate, "fromX", context)?.unwrap_or(0.0);
                    let from_y =
                        Self::get_number_property(&translate, "fromY", context)?.unwrap_or(0.0);
                    let to_x =
                        Self::get_number_property(&translate, "toX", context)?.unwrap_or(0.0);
                    let to_y =
                        Self::get_number_property(&translate, "toY", context)?.unwrap_or(0.0);

                    properties.push(AnimatedProperty::Translate {
                        from_x: from_x as f32,
                        from_y: from_y as f32,
                        to_x: to_x as f32,
                        to_y: to_y as f32,
                    });
                }

                // Handle rotate property
                if let Ok(rotate) = Self::get_object_property(&props_val, "rotate", context) {
                    if let (Some(from), Some(to)) = (
                        Self::get_number_property(&rotate, "from", context)?,
                        Self::get_number_property(&rotate, "to", context)?,
                    ) {
                        properties.push(AnimatedProperty::Rotate {
                            from: from as f32,
                            to: to as f32,
                        });
                    }
                }
            }
        }

        Ok(properties)
    }

    /// Helper: Get object property
    fn get_object_property(obj: &JsValue, name: &str, context: &mut Context) -> JsResult<JsValue> {
        if let Some(object) = obj.as_object() {
            object.get(js_string!(name), context)
        } else {
            Ok(JsValue::undefined())
        }
    }

    /// Execute JavaScript animation code
    pub fn execute(&mut self, code: &str) -> Result<(), String> {
        let source = Source::from_bytes(code);
        match self.context.eval(source) {
            Ok(result) => {
                tracing::debug!("JS execution result: {:?}", result);
                Ok(())
            }
            Err(e) => {
                let msg = format!("JavaScript error: {:?}", e);
                tracing::error!("{}", msg);
                Err(msg)
            }
        }
    }

    /// Tick all animations and return effects
    pub fn tick(&mut self) -> Vec<Effect> {
        self.engine.tick(self.viewport)
    }

    /// Add an animation programmatically
    pub fn add_animation(&mut self, animation: Animation) -> usize {
        self.engine.add_animation(animation)
    }

    /// Start an animation by ID
    pub fn start(&mut self, id: usize) -> bool {
        self.engine.start(id)
    }

    /// Start all pending animations
    pub fn start_all(&mut self) {
        for i in 0..self.engine.animations.len() {
            if let Some(anim) = self.engine.get_animation(i) {
                if anim.state == super::AnimationState::Pending {
                    anim.start();
                }
            }
        }
    }

    /// Get the Boa context for advanced usage
    pub fn context(&mut self) -> &mut Context {
        &mut self.context
    }

    /// Get mutable access to the animation engine
    pub fn engine(&mut self) -> &mut AnimationEngine {
        &mut self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_runtime_creation() {
        let _runtime = JsAnimationRuntime::new((800, 600));
    }

    #[test]
    fn test_js_execute() {
        let mut runtime = JsAnimationRuntime::new((800, 600));
        let result = runtime.execute(r#"print("Hello from JS!")"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_js_ease_function() {
        let mut runtime = JsAnimationRuntime::new((800, 600));
        let result = runtime.execute(
            r#"
            const eased = mustang.ease("quad-in", 0.5);
            print("Eased value: " + eased);
        "#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_js_animate_function() {
        let mut runtime = JsAnimationRuntime::new((800, 600));
        let result = runtime.execute(
            r#"
            const anim = mustang.animate({
                selector: ".my-element",
                duration: 500,
                easing: "ease-out"
            });
            print("Animation created for: " + anim.selector);
        "#,
        );
        assert!(result.is_ok());
    }
}
