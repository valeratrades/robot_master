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
exported = torch.export.export(
    wrapper,
    (dummy,),
    dynamic_shapes=({0: batch_dim},),
    strict=False,
)

buf = io.BytesIO()
torch.onnx.export(exported, buf, input_names=['state'], output_names=['policy','value'])
buf.seek(0)
sess = ort.InferenceSession(buf.read())
print('Input shapes:', [i.shape for i in sess.get_inputs()])

big = torch.randn(128, in_channels(5), 5, 5)
with torch.no_grad():
    pt_p, pt_v = wrapper(big)
onnx_p, onnx_v = sess.run(None, {'state': big.numpy()})
print('batch=128 diff=%e' % np.abs(pt_p.numpy()-onnx_p).max())

with torch.no_grad():
    pt_p1, _ = wrapper(dummy)
onnx_p1, _ = sess.run(None, {'state': dummy.numpy()})
print('batch=1  diff=%e' % np.abs(pt_p1.numpy()-onnx_p1).max())
