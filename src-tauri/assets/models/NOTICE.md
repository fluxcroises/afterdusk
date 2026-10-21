# Modèles de détection des yeux fermés

Livrés avec Afterdusk, exécutés sur la machine par ONNX Runtime (décision 0010). Rien n'est envoyé en ligne.

| Fichier                             | Rôle                                           | Licence                                                  | Source                                                                                                 | SHA-256                                                            |
| ----------------------------------- | ---------------------------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------ |
| `face_detection_yunet_2023mar.onnx` | visages et position des yeux (YuNet)           | MIT, voir `LICENSE-yunet.txt`                            | `opencv/opencv_zoo`, `models/face_detection_yunet`, issu de libfacedetection (Shiqi Yu)                | `8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4` |
| `face_landmarks.onnx`               | 478 repères du visage (MediaPipe Face Mesh V2) | Apache 2.0 (code et poids), voir `LICENSE-mediapipe.txt` | conversion ONNX du modèle MediaPipe de Google, Hugging Face `fernandotonon/QtMeshEditor-facemesh-onnx` | `d16e5a55e6a480284d468ee32469692464049518c311662ab3956681de31e3e9` |

Les fichiers ne sont pas modifiés.
