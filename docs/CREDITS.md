# Credits And Attribution

QATQ is an independent research implementation. It should credit the upstream
ideas it builds on and avoid implying that its current code is an official
implementation of any external paper.

## TurboQuant

The TurboQuant concept should be credited to the Google Research / Google
DeepMind / NYU work:

- Amir Zandieh, Majid Daliri, Majid Hadian, and Vahab Mirrokni,
  "TurboQuant: Online Vector Quantization with Near-optimal Distortion Rate",
  arXiv:2504.19874.
- The Google Research blog post "TurboQuant: Redefining AI efficiency with
  extreme compression", published March 24, 2026.

The QATQ `turboquant-q4` mode is a reference base-comparator path. It implements
deterministic data-oblivious orthogonal rotation, scalar q4 quantization, and a
structured QJL residual sign estimator for query-side inner products. It is not
an official Google implementation.

## Quaternion Lineage

The mathematical quaternion and Hamilton-product foundation should credit
William Rowan Hamilton, who introduced quaternions in 1843.

The neural-network motivation for treating related multidimensional channels as
quaternion entities should also credit prior quaternion neural-network work,
including:

- Titouan Parcollet, Mirco Ravanelli, Mohamed Morchid, Georges Linarès, Chiheb
  Trabelsi, Renato De Mori, and Yoshua Bengio, "Quaternion Recurrent Neural
  Networks", arXiv:1806.04418.
- Titouan Parcollet, Ying Zhang, Mohamed Morchid, Chiheb Trabelsi, Georges
  Linarès, Renato De Mori, and Yoshua Bengio, "Quaternion Convolutional Neural
  Networks for End-to-End Automatic Speech Recognition", arXiv:1806.07789.

QATQ's `phase1-q4` mode is the quaternion-augmented overlay path. It groups four
coordinates into quaternion lanes, applies deterministic Hamilton-product
rotation, scalar q4 quantization, and a compact residual-sign experiment.
