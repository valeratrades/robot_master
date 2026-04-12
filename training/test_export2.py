import torch, io, numpy as np, onnxruntime as ort
from model_transformer import RobotMasterTransformer, in_channels

model = RobotMasterTransformer(board_size=5)
model.eval()

class ExportWrapper(torch.nn.Module):
    def __init__(self, inner):
        super().__init__()
        self.inner = inner
    def forward(self, x):
        policy, _policy_soft, value = self.inner(x)
        return policy, value

wrapper = ExportWrapper(model)
wrapper.eval()
dummy = torch.randn(1, in_channels(5), 5, 5)

batch_dim = torch.export.Dim('batch')
ep = torch.export.draft_export(
    wrapper,
    (dummy,),
    dynamic_shapes=({0: batch_dim},),
    strict=False,
)
print("Draft export done")
print("Dim violations:", ep.range_constraints)
