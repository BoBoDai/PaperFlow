//! Deterministic Chinese-to-English translation for common academic terms.
//!
//! Queries fully covered by the glossary work without an LLM (and therefore
//! without an API key). Anything left in Chinese is handed to the configured
//! LLM by the usecase layer.

use crate::is_cjk_char;

/// Common Chinese academic terms mapped to English search keywords.
///
/// Longer phrases must win over their substrings ("大语言模型" before
/// "语言模型"), so the list is sorted by phrase length at lookup time.
const GLOSSARY: &[(&str, &str)] = &[
    ("强化学习从人类反馈", "reinforcement learning from human feedback"),
    ("多模态大语言模型", "multimodal large language models"),
    ("预训练语言模型", "pretrained language models"),
    ("生成对抗网络", "generative adversarial networks"),
    ("对抗生成网络", "generative adversarial networks"),
    ("神经架构搜索", "neural architecture search"),
    ("图卷积网络", "graph convolutional networks"),
    ("检索增强生成", "retrieval-augmented generation"),
    ("变分自编码器", "variational autoencoders"),
    ("时间序列预测", "time series forecasting"),
    ("三维重建", "3d reconstruction"),
    ("神经辐射场", "neural radiance fields"),
    ("大语言模型", "large language models"),
    ("大型语言模型", "large language models"),
    ("视觉语言模型", "vision-language models"),
    ("多模态", "multimodal"),
    ("扩散模型", "diffusion models"),
    ("生成模型", "generative models"),
    ("语言模型", "language models"),
    ("图神经网络", "graph neural networks"),
    ("神经网络", "neural networks"),
    ("计算机视觉", "computer vision"),
    ("自然语言处理", "natural language processing"),
    ("强化学习", "reinforcement learning"),
    ("深度学习", "deep learning"),
    ("机器学习", "machine learning"),
    ("人工智能", "artificial intelligence"),
    ("目标检测", "object detection"),
    ("图像分割", "image segmentation"),
    ("图像生成", "image generation"),
    ("视频生成", "video generation"),
    ("语音识别", "speech recognition"),
    ("推荐系统", "recommender systems"),
    ("知识图谱", "knowledge graph"),
    ("联邦学习", "federated learning"),
    ("迁移学习", "transfer learning"),
    ("自监督学习", "self-supervised learning"),
    ("无监督学习", "unsupervised learning"),
    ("半监督学习", "semi-supervised learning"),
    ("监督学习", "supervised learning"),
    ("对比学习", "contrastive learning"),
    ("元学习", "meta-learning"),
    ("多任务学习", "multi-task learning"),
    ("持续学习", "continual learning"),
    ("终身学习", "lifelong learning"),
    ("主动学习", "active learning"),
    ("课程学习", "curriculum learning"),
    ("模仿学习", "imitation learning"),
    ("在线学习", "online learning"),
    ("表征学习", "representation learning"),
    ("注意力机制", "attention mechanism"),
    ("注意力", "attention"),
    ("提示工程", "prompt engineering"),
    ("检索增强", "retrieval-augmented"),
    ("信息检索", "information retrieval"),
    ("机器翻译", "machine translation"),
    ("文本分类", "text classification"),
    ("情感分析", "sentiment analysis"),
    ("问答系统", "question answering"),
    ("对话系统", "dialogue systems"),
    ("代码生成", "code generation"),
    ("代码补全", "code completion"),
    ("知识蒸馏", "knowledge distillation"),
    ("模型压缩", "model compression"),
    ("量化", "quantization"),
    ("剪枝", "pruning"),
    ("超参数优化", "hyperparameter optimization"),
    ("贝叶斯优化", "bayesian optimization"),
    ("因果推断", "causal inference"),
    ("时间序列", "time series"),
    ("医学图像", "medical imaging"),
    ("医疗影像", "medical imaging"),
    ("药物发现", "drug discovery"),
    ("遥感", "remote sensing"),
    ("机器人", "robotics"),
    ("多智能体", "multi-agent"),
    ("具身智能", "embodied intelligence"),
    ("自动驾驶", "autonomous driving"),
    ("点云", "point cloud"),
    ("深度估计", "depth estimation"),
    ("光流", "optical flow"),
    ("姿态估计", "pose estimation"),
    ("人脸识别", "face recognition"),
    ("行人重识别", "person re-identification"),
    ("异常检测", "anomaly detection"),
    ("对抗攻击", "adversarial attacks"),
    ("对抗样本", "adversarial examples"),
    ("安全对齐", "safety alignment"),
    ("零样本", "zero-shot"),
    ("少样本", "few-shot"),
    ("长文本", "long context"),
    ("大模型", "large models"),
    ("微调", "fine-tuning"),
    ("预训练", "pretraining"),
    ("智能体", "agents"),
    ("思维链", "chain-of-thought"),
    ("推理", "reasoning"),
    ("对齐", "alignment"),
    ("架构", "architecture"),
    ("优化", "optimization"),
    ("训练", "training"),
    ("预测", "prediction"),
    ("检测", "detection"),
    ("分割", "segmentation"),
    ("分类", "classification"),
    ("生成", "generation"),
    ("检索", "retrieval"),
    ("数据集", "dataset"),
    ("基准测试", "benchmark"),
    ("鲁棒性", "robustness"),
    ("可解释性", "interpretability"),
    ("隐私", "privacy"),
    ("安全", "security"),
    ("公平性", "fairness"),
    ("偏差", "bias"),
    ("数据库", "database"),
    ("分布式", "distributed"),
    ("编译器", "compilers"),
    ("程序合成", "program synthesis"),
    ("形式化验证", "formal verification"),
    ("软件工程", "software engineering"),
    ("网络安全", "cybersecurity"),
    ("密码学", "cryptography"),
    ("区块链", "blockchain"),
    ("量子计算", "quantum computing"),
    ("量子", "quantum"),
];

/// Chinese words that carry no search signal in a paper query.
const STOPWORDS: &[&str] = &[
    "最新", "最近", "论文", "文章", "研究", "关于", "有关", "方法", "技术", "应用",
    "进展", "综述", "一个", "一些", "如何", "什么", "哪些", "的", "了", "在", "中",
    "与", "和", "及", "对", "基于", "用于",
];

/// Translate a Chinese query using the local glossary.
///
/// Returns a space-separated English keyword string. The result may be empty
/// when the query contains only unknown Chinese terms; callers should hand
/// that case to the LLM.
pub fn translate_query_locally(query: &str) -> String {
    let mut text = query.to_lowercase();

    let mut glossary = GLOSSARY.to_vec();
    glossary.sort_by_key(|(chinese, _)| std::cmp::Reverse(chinese.chars().count()));

    for (chinese, english) in glossary {
        if text.contains(chinese) {
            text = text.replace(chinese, &format!(" {english} "));
        }
    }

    for stopword in STOPWORDS {
        if text.contains(stopword) {
            text = text.replace(stopword, " ");
        }
    }

    text.chars()
        .filter(|c| {
            !is_cjk_char(*c)
                && (c.is_ascii_alphanumeric()
                    || c.is_whitespace()
                    || *c == '-'
                    || *c == '+'
                    || *c == '.')
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_common_terms() {
        assert_eq!(translate_query_locally("扩散模型"), "diffusion models");
        assert_eq!(
            translate_query_locally("最新的扩散模型论文"),
            "diffusion models"
        );
        assert_eq!(
            translate_query_locally("大语言模型 微调"),
            "large language models fine-tuning"
        );
        assert_eq!(
            translate_query_locally("图神经网络在药物发现中的应用"),
            "graph neural networks drug discovery"
        );
        assert_eq!(
            translate_query_locally("强化学习从人类反馈"),
            "reinforcement learning from human feedback"
        );
    }

    #[test]
    fn keeps_english_terms_in_mixed_queries() {
        assert_eq!(
            translate_query_locally("Transformer 架构"),
            "transformer architecture"
        );
        assert_eq!(
            translate_query_locally("扩散模型 in medical imaging"),
            "diffusion models in medical imaging"
        );
    }

    #[test]
    fn returns_empty_for_unknown_chinese() {
        assert_eq!(translate_query_locally("冷门未知术语"), "");
        assert_eq!(translate_query_locally("   "), "");
    }
}
