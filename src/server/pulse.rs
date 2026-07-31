@@
-        let name = sink_input.proplist.get_str("application.name").unwrap_or_default();
-        let description = sink_input.name.as_ref().map(Cow::to_string).unwrap_or_default();
+        // Prefer stable programmatic identifier fields where available. Fall back to human-friendly
+        // sink_input.name (description) only if no other id is present. This ensures loopback
+        // and filter-chain nodes (which set media.name/node.name) expose a usable `name` for
+        // filtering and programmatic identification.
+        let app_name = sink_input.proplist.get_str("application.name");
+        let media_name = sink_input.proplist.get_str("media.name");
+        let node_name = sink_input.proplist.get_str("node.name");
+        let fallback_name = sink_input.name.as_ref().map(Cow::as_ref);
+
+        let name = app_name
+            .or(media_name)
+            .or(node_name)
+            .or(fallback_name)
+            .unwrap_or_default()
+            .to_string();
+
+        // Keep description for UI-friendly text (still useful when name is an internal id)
+        let description = sink_input.name.as_ref().map(Cow::to_string).unwrap_or_default();
@@
         OutputClient {
             id: sink_input.index,
             process,
             name,
             description,
             icon,
             volume,
             max_volume: 2.55,
             muted: sink_input.mute,
             corked: sink_input.corked,
             kind: Kind::Out | Kind::Software,
         }
