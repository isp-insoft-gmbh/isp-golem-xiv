// Golem-owned Pi extension, baked into the agent image at /opt/golem/pi-report.js.
// Its single successful call is the run's typed result; the runner reads it from the
// JSON event stream and never infers an outcome from prose. No imports: Pi validates
// the plain JSON Schema below before execute() runs.
export default function (pi) {
  pi.registerTool({
    name: 'golem_report',
    label: 'Golem report',
    description: 'Report the final result of this golem run. Call it exactly once, as your last action.',
    promptSnippet: 'Report the final disposition and complete answer of this golem run',
    promptGuidelines: [
      'Call golem_report exactly once, as your last action; text outside golem_report is not delivered.',
      'Use golem_report disposition blocked if policy, permissions or another obstacle prevented fulfilling the task; otherwise completed, including ordinary answers or nothing to do.',
    ],
    parameters: {
      type: 'object',
      properties: {
        disposition: {
          type: 'string',
          enum: ['completed', 'blocked'],
          description: 'blocked if policy, permissions or another obstacle prevented fulfilling the task; otherwise completed, including ordinary answers or nothing to do.',
        },
        message: { type: 'string', minLength: 1, maxLength: 48000, description: 'Your complete final answer.' },
      },
      required: ['disposition', 'message'],
      additionalProperties: false,
    },
    async execute() {
      return { content: [{ type: 'text', text: 'Report recorded.' }], details: undefined, terminate: true };
    },
  });
}
