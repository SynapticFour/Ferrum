cwlVersion: v1.0
class: Workflow
inputs:
  message:
    type: string
    default: smoke-pilot
outputs:
  out:
    type: File
    outputSource: echo_step/out
steps:
  echo_step:
    run:
      cwlVersion: v1.0
      class: CommandLineTool
      baseCommand: echo
      inputs:
        message:
          type: string
          inputBinding:
            position: 1
      outputs:
        out:
          type: stdout
      requirements:
        DockerRequirement:
          dockerPull: alpine:3.20
    in:
      message: message
    out: [out]
