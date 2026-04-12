import torch, numpy as np, onnxruntime as ort
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
dummy = torch.randn(2, in_channels(5), 5, 5)

batch_dim = torch.export.Dim('batch')
ep = torch.export.export(wrapper, (dummy,), dynamic_shapes=({0: batch_dim},), strict=False)
print("export done")

onnx_prog = torch.onnx.export(ep, input_names=['state'], output_names=['policy','value'], opset_version=18)
onnx_prog.save('/tmp/test_dynamic.onnx')

sess = ort.InferenceSession('/tmp/test_dynamic.onnx')
print('Input shapes:', [i.shape for i in sess.get_inputs()])

for batch_size in [1, 2, 128]:
    x = torch.randn(batch_size, in_channels(5), 5, 5)
    with torch.no_grad():
        pt_p, pt_v = wrapper(x)
    onnx_p, onnx_v = sess.run(None, {'state': x.numpy()})
    diff = np.abs(pt_p.numpy()-onnx_p).max()
    print(f'batch={batch_size} diff={diff:.2e}')
