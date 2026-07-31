    fn handle_msg_output(&mut self, msg: MessageOutput) {
        match msg {
            MessageOutput::New(output) => {
                // Apply filter to outputs (sink nodes). Match against both name and port.
                if should_show(&output.name, &self.filter) || should_show(&output.port, &self.filter) {
                    self.switches.push(output);
                }
            },
            MessageOutput::Master(output) => {
                if should_show(&output.name, &self.filter) || should_show(&output.port, &self.filter) {
                    self.switches.set_active(output);
                }
            }
        }
    }
