// Dependency management commands

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub install_command: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CudaStatus {
    pub available: bool,
    pub version: Option<String>,
    pub device_name: Option<String>,
    pub device_count: u32,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptionalDependenciesStatus {
    pub huggingface_hub: DependencyStatus,
    pub hf_xet: DependencyStatus,
    pub psutil: DependencyStatus,
    pub safetensors: DependencyStatus,
    pub peft: DependencyStatus,
    pub trl: DependencyStatus,
    pub bitsandbytes: DependencyStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DependenciesStatus {
    pub python: DependencyStatus,
    pub pip: DependencyStatus,
    pub transformers: DependencyStatus,
    pub datasets: DependencyStatus,
    pub torch: DependencyStatus,
    pub accelerate: DependencyStatus,
    pub cuda: CudaStatus,
    pub optional: OptionalDependenciesStatus,
}

#[tauri::command]
pub async fn check_dependencies() -> Result<DependenciesStatus, String> {
    // Check Python - prefer 3.11 or 3.12 for CUDA support
    let (python_cmd, python_args) = {
        #[cfg(windows)]
        {
            // On Windows, try py launcher with specific versions first
            if Command::new("py").arg("-3.11").arg("--version").output().is_ok() {
                ("py", Some("-3.11"))
            } else if Command::new("py").arg("-3.12").arg("--version").output().is_ok() {
                ("py", Some("-3.12"))
            } else if Command::new("py").arg("--version").output().is_ok() {
                ("py", Some("-3"))
            } else if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Ok(DependenciesStatus {
                    python: DependencyStatus {
                        name: "Python".to_string(),
                        installed: false,
                        version: None,
                        install_command: "Download from python.org".to_string(),
                        description: "Python 3.8+ is required for model training".to_string(),
                    },
                    pip: DependencyStatus {
                        name: "pip".to_string(),
                        installed: false,
                        version: None,
                        install_command: "Install Python first".to_string(),
                        description: "Python package manager".to_string(),
                    },
                    transformers: DependencyStatus {
                        name: "transformers".to_string(),
                        installed: false,
                        version: None,
                        install_command: "pip install transformers".to_string(),
                        description: "Hugging Face transformers library for model training".to_string(),
                    },
                    datasets: DependencyStatus {
                        name: "datasets".to_string(),
                        installed: false,
                        version: None,
                        install_command: "pip install datasets".to_string(),
                        description: "Hugging Face datasets library".to_string(),
                    },
                    torch: DependencyStatus {
                        name: "torch".to_string(),
                        installed: false,
                        version: None,
                        install_command: "pip install torch".to_string(),
                        description: "PyTorch deep learning framework".to_string(),
                    },
                    accelerate: DependencyStatus {
                        name: "accelerate".to_string(),
                        installed: false,
                        version: None,
                        install_command: "pip install accelerate>=0.26.0".to_string(),
                        description: "Required for PyTorch Trainer (distributed training support)".to_string(),
                    },
                    cuda: CudaStatus {
                        available: false,
                        version: None,
                        device_name: None,
                        device_count: 0,
                        message: "Python is not installed. Install Python first to check CUDA support.".to_string(),
                    },
                    optional: OptionalDependenciesStatus {
                        huggingface_hub: DependencyStatus {
                            name: "huggingface_hub".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install huggingface_hub".to_string(),
                            description: "Required for downloading models from Hugging Face".to_string(),
                        },
                        hf_xet: DependencyStatus {
                            name: "hf_xet".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install huggingface_hub[hf_xet]".to_string(),
                            description: "Optional: Faster downloads from Hugging Face (Xet Storage)".to_string(),
                        },
                        psutil: DependencyStatus {
                            name: "psutil".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install psutil".to_string(),
                            description: "Used for CPU/memory monitoring during training".to_string(),
                        },
                        safetensors: DependencyStatus {
                            name: "safetensors".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install safetensors".to_string(),
                            description: "Required for loading models with PyTorch < 2.6".to_string(),
                        },
                        peft: DependencyStatus {
                            name: "peft".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install peft".to_string(),
                            description: "Parameter-Efficient Fine-Tuning (LoRA, QLoRA)".to_string(),
                        },
                        trl: DependencyStatus {
                            name: "trl".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install trl".to_string(),
                            description: "Transformer Reinforcement Learning (SFT, DPO, RLHF)".to_string(),
                        },
                        bitsandbytes: DependencyStatus {
                            name: "bitsandbytes".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install bitsandbytes".to_string(),
                            description: "8-bit/4-bit quantization for memory-efficient training".to_string(),
                        },
                    },
                });
            }
        }
        #[cfg(not(windows))]
        {
            if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Ok(DependenciesStatus {
                    python: DependencyStatus {
                        name: "Python".to_string(),
                        installed: false,
                        version: None,
                        install_command: "Download from python.org".to_string(),
                        description: "Python 3.8+ is required for model training".to_string(),
                    },
                    pip: DependencyStatus {
                        name: "pip".to_string(),
                        installed: false,
                        version: None,
                        install_command: "Install Python first".to_string(),
                        description: "Python package manager".to_string(),
                    },
                    transformers: DependencyStatus {
                        name: "transformers".to_string(),
                        installed: false,
                        version: None,
                        install_command: "pip install transformers".to_string(),
                        description: "Hugging Face transformers library for model training".to_string(),
                    },
                    datasets: DependencyStatus {
                        name: "datasets".to_string(),
                        installed: false,
                        version: None,
                        install_command: "pip install datasets".to_string(),
                        description: "Hugging Face datasets library".to_string(),
                    },
                    torch: DependencyStatus {
                        name: "torch".to_string(),
                        installed: false,
                        version: None,
                        install_command: "pip install torch".to_string(),
                        description: "PyTorch deep learning framework".to_string(),
                    },
                    accelerate: DependencyStatus {
                        name: "accelerate".to_string(),
                        installed: false,
                        version: None,
                        install_command: "pip install accelerate>=0.26.0".to_string(),
                        description: "Required for PyTorch Trainer (distributed training support)".to_string(),
                    },
                    cuda: CudaStatus {
                        available: false,
                        version: None,
                        device_name: None,
                        device_count: 0,
                        message: "Python is not installed. Install Python first to check CUDA support.".to_string(),
                    },
                    optional: OptionalDependenciesStatus {
                        huggingface_hub: DependencyStatus {
                            name: "huggingface_hub".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install huggingface_hub".to_string(),
                            description: "Required for downloading models from Hugging Face".to_string(),
                        },
                        hf_xet: DependencyStatus {
                            name: "hf_xet".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install huggingface_hub[hf_xet]".to_string(),
                            description: "Optional: Faster downloads from Hugging Face (Xet Storage)".to_string(),
                        },
                        psutil: DependencyStatus {
                            name: "psutil".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install psutil".to_string(),
                            description: "Used for CPU/memory monitoring during training".to_string(),
                        },
                        safetensors: DependencyStatus {
                            name: "safetensors".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install safetensors".to_string(),
                            description: "Required for loading models with PyTorch < 2.6".to_string(),
                        },
                        peft: DependencyStatus {
                            name: "peft".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install peft".to_string(),
                            description: "Parameter-Efficient Fine-Tuning (LoRA, QLoRA)".to_string(),
                        },
                        trl: DependencyStatus {
                            name: "trl".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install trl".to_string(),
                            description: "Transformer Reinforcement Learning (SFT, DPO, RLHF)".to_string(),
                        },
                        bitsandbytes: DependencyStatus {
                            name: "bitsandbytes".to_string(),
                            installed: false,
                            version: None,
                            install_command: "pip install bitsandbytes".to_string(),
                            description: "8-bit/4-bit quantization for memory-efficient training".to_string(),
                        },
                    },
                });
            }
        }
    };
    
    // Helper to run Python commands
    let run_python = |args: &[&str]| -> std::io::Result<std::process::Output> {
        let mut cmd = Command::new(python_cmd);
        if let Some(version_arg) = python_args {
            cmd.arg(version_arg);
        }
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output()
    };
    
    // Helper to run pip commands
    let run_pip = |args: &[&str]| -> std::io::Result<std::process::Output> {
        if python_cmd == "py" {
            let mut cmd = Command::new("py");
            if let Some(version_arg) = python_args {
                cmd.arg(version_arg);
            }
            cmd.arg("-m").arg("pip");
            for arg in args {
                cmd.arg(arg);
            }
            cmd.output()
        } else {
            let pip_cmd = if python_cmd == "python3" { "pip3" } else { "pip" };
            let mut cmd = Command::new(pip_cmd);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.output()
        }
    };
    
    // Get Python version
    let python_version = run_python(&["--version"])
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|v| v.trim().to_string());
    
    // Check if Python version is 3.13 (which may not have CUDA wheels yet)
    let is_python_313 = python_version.as_ref()
        .map(|v| v.contains("3.13"))
        .unwrap_or(false);

    // Check pip
    let pip_installed = run_pip(&["--version"]).is_ok();
    let pip_version = if pip_installed {
        run_pip(&["--version"])
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|v| v.trim().to_string())
    } else {
        None
    };

    // Check Python packages
    let check_package = |package_name: &str| -> (bool, Option<String>) {
        if !pip_installed {
            return (false, None);
        }
        
        let output = run_pip(&["show", package_name]).ok();
        
        if let Some(output) = output {
            if output.status.success() {
                // Extract version from output
                let stdout = String::from_utf8_lossy(&output.stdout);
                let version = stdout
                    .lines()
                    .find(|line| line.starts_with("Version:"))
                    .and_then(|line| line.split(':').nth(1))
                    .map(|v| v.trim().to_string());
                return (true, version);
            }
        }
        (false, None)
    };

    let (transformers_installed, transformers_version) = check_package("transformers");
    let (datasets_installed, datasets_version) = check_package("datasets");
    let (torch_installed, torch_version) = check_package("torch");
    let (accelerate_installed, accelerate_version) = check_package("accelerate");
    
    // Check optional dependencies
    let (huggingface_hub_installed, huggingface_hub_version) = check_package("huggingface_hub");
    let (hf_xet_installed, _hf_xet_version) = check_package("hf_xet");
    let (psutil_installed, psutil_version) = check_package("psutil");
    let (safetensors_installed, safetensors_version) = check_package("safetensors");
    let (peft_installed, peft_version) = check_package("peft");
    let (trl_installed, trl_version) = check_package("trl");
    let (bitsandbytes_installed, bitsandbytes_version) = check_package("bitsandbytes");
    
    // Create pip command string for install commands
    let pip_cmd_str = if python_cmd == "py" {
        if let Some(version) = python_args {
            format!("py {} -m pip", version)
        } else {
            "py -3 -m pip".to_string()
        }
    } else if python_cmd == "python3" {
        "pip3".to_string()
    } else {
        "pip".to_string()
    };

    // Check CUDA availability
    let cuda_status = if torch_installed {
        // Try to check CUDA via Python with better error handling
        let cuda_script = "import torch\nimport sys\ntry:\n    cuda_available = torch.cuda.is_available()\n    print('CUDA_AVAILABLE:', cuda_available)\n    if cuda_available:\n        print('CUDA_VERSION:', torch.version.cuda)\n        print('DEVICE_COUNT:', torch.cuda.device_count())\n        if torch.cuda.device_count() > 0:\n            print('DEVICE_NAME:', torch.cuda.get_device_name(0))\n        else:\n            print('DEVICE_NAME: N/A')\n    else:\n        print('CUDA_VERSION: N/A')\n        print('DEVICE_COUNT: 0')\n        print('DEVICE_NAME: N/A')\n    print('TORCH_CUDA_BUILT:', torch.cuda.is_available() or hasattr(torch.version, 'cuda') and torch.version.cuda is not None)\nexcept Exception as e:\n    print('ERROR:', str(e), file=sys.stderr)\n    sys.exit(1)\n";
        let cuda_check = run_python(&["-c", cuda_script]);
        
        if let Ok(output) = cuda_check {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let _stderr = String::from_utf8_lossy(&output.stderr);
                let mut available = false;
                let mut version = None;
                let mut device_count = 0;
                let mut device_name = None;
                let mut torch_cuda_built = false;
                
                for line in stdout.lines() {
                    if line.starts_with("CUDA_AVAILABLE:") {
                        available = line.split(':').nth(1).map(|s| s.trim() == "True").unwrap_or(false);
                    } else if line.starts_with("CUDA_VERSION:") {
                        version = line.split(':').nth(1).map(|s| s.trim().to_string()).filter(|s| s != "N/A" && !s.is_empty());
                    } else if line.starts_with("DEVICE_COUNT:") {
                        device_count = line.split(':').nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                    } else if line.starts_with("DEVICE_NAME:") {
                        device_name = line.split(':').nth(1).map(|s| s.trim().to_string()).filter(|s| s != "N/A" && !s.is_empty());
                    } else if line.starts_with("TORCH_CUDA_BUILT:") {
                        torch_cuda_built = line.split(':').nth(1).map(|s| s.trim() == "True").unwrap_or(false);
                    }
                }
                
                let message = if available {
                    format!("✅ CUDA is available! GPU training will be used. Device: {} ({} device(s))", 
                        device_name.as_ref().unwrap_or(&"Unknown".to_string()), device_count)
                } else if is_python_313 {
                    "⚠️ Python 3.13 detected. PyTorch does not yet have CUDA wheels for Python 3.13.\n\nOptions:\n1. Use Python 3.11 or 3.12 for CUDA support (recommended)\n2. Continue with CPU-only PyTorch (slower but works)\n3. Wait for PyTorch to add Python 3.13 CUDA support".to_string()
                } else if torch_cuda_built {
                    "⚠️ PyTorch was built with CUDA support, but CUDA is not available. Make sure:\n1. NVIDIA drivers are installed\n2. CUDA toolkit is installed\n3. GPU is detected by the system".to_string()
                } else {
                    "⚠️ PyTorch CPU-only version detected. CUDA is not available. To enable GPU training, uninstall PyTorch and reinstall with CUDA support:\npip uninstall torch torchvision torchaudio\npip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121".to_string()
                };
                
                CudaStatus {
                    available,
                    version,
                    device_name,
                    device_count,
                    message,
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let error_msg = if !stderr.is_empty() {
                    format!("Error checking CUDA: {}", stderr.trim())
                } else if !stdout.is_empty() {
                    format!("Error checking CUDA: {}", stdout.trim())
                } else {
                    "Could not check CUDA status. PyTorch may not be properly installed.".to_string()
                };
                
                CudaStatus {
                    available: false,
                    version: None,
                    device_name: None,
                    device_count: 0,
                    message: error_msg,
                }
            }
        } else {
            CudaStatus {
                available: false,
                version: None,
                device_name: None,
                device_count: 0,
                message: format!("Could not execute Python command '{}' to check CUDA status. Make sure Python is in your PATH.", python_cmd),
            }
        }
    } else {
        CudaStatus {
            available: false,
            version: None,
            device_name: None,
            device_count: 0,
            message: "PyTorch is not installed. Install PyTorch first to check CUDA support.".to_string(),
        }
    };

    Ok(DependenciesStatus {
        python: DependencyStatus {
            name: "Python".to_string(),
            installed: true,
            version: python_version,
            install_command: format!("{} --version", python_cmd),
            description: "Python 3.8+ is required for model training".to_string(),
        },
        pip: DependencyStatus {
            name: "pip".to_string(),
            installed: pip_installed,
            version: pip_version,
            install_command: if pip_installed {
                format!("{} --version", pip_cmd_str)
            } else {
                if python_cmd == "py" {
                    if let Some(version) = python_args {
                        format!("py {} -m ensurepip --upgrade", version)
                    } else {
                        "py -3 -m ensurepip --upgrade".to_string()
                    }
                } else {
                    format!("{} -m ensurepip --upgrade", python_cmd)
                }
            },
            description: "Python package manager".to_string(),
        },
        transformers: DependencyStatus {
            name: "transformers".to_string(),
            installed: transformers_installed,
            version: transformers_version,
            install_command: format!("{} install transformers", pip_cmd_str),
            description: "Hugging Face transformers library for model training".to_string(),
        },
        datasets: DependencyStatus {
            name: "datasets".to_string(),
            installed: datasets_installed,
            version: datasets_version,
            install_command: format!("{} install datasets", pip_cmd_str),
            description: "Hugging Face datasets library".to_string(),
        },
        torch: DependencyStatus {
            name: "torch".to_string(),
            installed: torch_installed,
            version: torch_version.clone(),
            install_command: format!("{} install --upgrade torch", pip_cmd_str),
            description: {
                let mut desc = "PyTorch deep learning framework".to_string();
                if let Some(ref v) = torch_version {
                    // Check if version is < 2.6
                    let version_ok = v.starts_with("2.6") || v.starts_with("2.7") || 
                                     v.starts_with("2.8") || v.starts_with("2.9") || 
                                     v.starts_with("3.");
                    if !version_ok {
                        desc.push_str(" (⚠️ Version 2.6+ required for training, or install safetensors)");
                    }
                }
                desc
            },
        },
        accelerate: DependencyStatus {
            name: "accelerate".to_string(),
            installed: accelerate_installed,
            version: accelerate_version,
            install_command: format!("{} install accelerate>=0.26.0", pip_cmd_str),
            description: "Required for PyTorch Trainer (distributed training support)".to_string(),
        },
        cuda: cuda_status,
        optional: OptionalDependenciesStatus {
            huggingface_hub: DependencyStatus {
                name: "huggingface_hub".to_string(),
                installed: huggingface_hub_installed,
                version: huggingface_hub_version,
                install_command: format!("{} install huggingface_hub", pip_cmd_str),
                description: "Required for downloading models from Hugging Face".to_string(),
            },
            hf_xet: DependencyStatus {
                name: "hf_xet".to_string(),
                installed: hf_xet_installed,
                version: None,
                install_command: format!("{} install huggingface_hub[hf_xet]", pip_cmd_str),
                description: "Optional: Faster downloads from Hugging Face (Xet Storage)".to_string(),
            },
            psutil: DependencyStatus {
                name: "psutil".to_string(),
                installed: psutil_installed,
                version: psutil_version,
                install_command: format!("{} install psutil", pip_cmd_str),
                description: "Used for CPU/memory monitoring during training".to_string(),
            },
            safetensors: DependencyStatus {
                name: "safetensors".to_string(),
                installed: safetensors_installed,
                version: safetensors_version,
                install_command: format!("{} install safetensors", pip_cmd_str),
                description: "Required for loading models with PyTorch < 2.6 (avoids torch.load security restrictions)".to_string(),
            },
            peft: DependencyStatus {
                name: "peft".to_string(),
                installed: peft_installed,
                version: peft_version,
                install_command: format!("{} install peft", pip_cmd_str),
                description: "Parameter-Efficient Fine-Tuning (LoRA, QLoRA)".to_string(),
            },
            trl: DependencyStatus {
                name: "trl".to_string(),
                installed: trl_installed,
                version: trl_version,
                install_command: format!("{} install trl", pip_cmd_str),
                description: "Transformer Reinforcement Learning (SFT, DPO, RLHF)".to_string(),
            },
            bitsandbytes: DependencyStatus {
                name: "bitsandbytes".to_string(),
                installed: bitsandbytes_installed,
                version: bitsandbytes_version,
                install_command: format!("{} install bitsandbytes", pip_cmd_str),
                description: "8-bit/4-bit quantization for memory-efficient training".to_string(),
            },
        },
    })
}

#[tauri::command]
pub async fn check_system_cuda() -> Result<serde_json::Value, String> {
    // Try to detect CUDA version from nvidia-smi
    let nvidia_smi_check = Command::new("nvidia-smi")
        .arg("--query-gpu=driver_version,cuda_version")
        .arg("--format=csv,noheader")
        .output();
    
    let mut system_cuda_version = None;
    let mut driver_version = None;
    let mut gpu_detected = false;
    
    if let Ok(output) = nvidia_smi_check {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = stdout.lines().next() {
                let parts: Vec<&str> = first_line.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    driver_version = Some(parts[0].to_string());
                    system_cuda_version = Some(parts[1].to_string());
                    gpu_detected = true;
                }
            }
        }
    }
    
    // Also try to get CUDA version from nvcc if available
    let nvcc_check = Command::new("nvcc")
        .arg("--version")
        .output();
    
    let mut nvcc_cuda_version = None;
    if let Ok(output) = nvcc_check {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("release") {
                    // Extract version like "release 11.8"
                    if let Some(version_part) = line.split("release").nth(1) {
                        let version = version_part.trim().split(',').next().unwrap_or("").trim();
                        if !version.is_empty() {
                            nvcc_cuda_version = Some(version.to_string());
                            break;
                        }
                    }
                }
            }
        }
    }
    
    Ok(serde_json::json!({
        "gpu_detected": gpu_detected,
        "driver_version": driver_version,
        "system_cuda_version": system_cuda_version,
        "nvcc_cuda_version": nvcc_cuda_version,
        "message": if gpu_detected {
            format!("GPU detected! Driver: {}, CUDA: {}", 
                driver_version.as_ref().unwrap_or(&"Unknown".to_string()),
                system_cuda_version.as_ref().unwrap_or(&"Unknown".to_string()))
        } else {
            "No GPU detected or nvidia-smi not available. Make sure NVIDIA drivers are installed.".to_string()
        }
    }))
}

#[tauri::command]
pub async fn install_dependency(dependency_name: String) -> Result<String, String> {
    // Use the same Python detection logic as check_dependencies
    let (python_cmd, python_args) = {
        #[cfg(windows)]
        {
            // On Windows, try py launcher with specific versions first
            if Command::new("py").arg("-3.11").arg("--version").output().is_ok() {
                ("py", Some("-3.11"))
            } else if Command::new("py").arg("-3.12").arg("--version").output().is_ok() {
                ("py", Some("-3.12"))
            } else if Command::new("py").arg("--version").output().is_ok() {
                ("py", Some("-3"))
            } else if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Err("Python is not installed. Please install Python 3.8+ first from python.org".to_string());
            }
        }
        #[cfg(not(windows))]
        {
            if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Err("Python is not installed. Please install Python 3.8+ first from python.org".to_string());
            }
        }
    };

    // Helper to run pip commands (same as in check_dependencies)
    let run_pip = |args: &[&str]| -> std::io::Result<std::process::Output> {
        if python_cmd == "py" {
            let mut cmd = Command::new("py");
            if let Some(version_arg) = python_args {
                cmd.arg(version_arg);
            }
            cmd.arg("-m").arg("pip");
            for arg in args {
                cmd.arg(arg);
            }
            cmd.output()
        } else {
            let pip_cmd = if python_cmd == "python3" { "pip3" } else { "pip" };
            let mut cmd = Command::new(pip_cmd);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.output()
        }
    };

    // Check if pip is available
    if run_pip(&["--version"]).is_err() {
        // Try to install pip first
        let install_pip = if python_cmd == "py" {
            let mut cmd = Command::new("py");
            if let Some(version_arg) = python_args {
                cmd.arg(version_arg);
            }
            cmd.arg("-m").arg("ensurepip").arg("--upgrade").output()
        } else {
            Command::new(python_cmd)
                .arg("-m")
                .arg("ensurepip")
                .arg("--upgrade")
                .output()
        };
        
        if install_pip.is_err() || !install_pip.unwrap().status.success() {
            return Err("pip is not available. Please install pip first.".to_string());
        }
    }

    // Install the requested package
    let package_name = match dependency_name.as_str() {
        "transformers" => "transformers",
        "datasets" => "datasets",
        "torch" => "torch",
        "accelerate" => "accelerate>=0.26.0",
        "huggingface_hub" => "huggingface_hub",
        "hf_xet" => "huggingface_hub[hf_xet]",
        "psutil" => "psutil",
        "safetensors" => "safetensors",
        "peft" => "peft",
        "trl" => "trl",
        "bitsandbytes" => "bitsandbytes",
        "pip" => {
            // Install pip using ensurepip
            let output = if python_cmd == "py" {
                let mut cmd = Command::new("py");
                if let Some(version_arg) = python_args {
                    cmd.arg(version_arg);
                }
                cmd.arg("-m").arg("ensurepip").arg("--upgrade").output()
            } else {
                Command::new(python_cmd)
                    .arg("-m")
                    .arg("ensurepip")
                    .arg("--upgrade")
                    .output()
            };
            
            let output = output.map_err(|e| format!("Failed to install pip: {}", e))?;
            
            if output.status.success() {
                return Ok("pip installed successfully".to_string());
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to install pip: {}", error));
            }
        }
        _ => return Err(format!("Unknown dependency: {}", dependency_name)),
    };

    // Execute pip install using the helper function
    let output = run_pip(&["install", package_name])
        .map_err(|e| format!("Failed to execute pip install: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(format!("{} installed successfully. {}", dependency_name, stdout))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to install {}: {}", dependency_name, stderr))
    }
}

#[tauri::command]
pub async fn install_all_dependencies() -> Result<String, String> {
    // Install all missing dependencies
    let deps = check_dependencies().await?;
    let mut results = Vec::new();
    let mut errors = Vec::new();

    // Install pip if needed
    if !deps.pip.installed {
        match install_dependency("pip".to_string()).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("pip: {}", e)),
        }
    }

    // Install transformers if needed
    if !deps.transformers.installed {
        match install_dependency("transformers".to_string()).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("transformers: {}", e)),
        }
    }

    // Install datasets if needed
    if !deps.datasets.installed {
        match install_dependency("datasets".to_string()).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("datasets: {}", e)),
        }
    }

    // Install torch if needed
    if !deps.torch.installed {
        match install_dependency("torch".to_string()).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("torch: {}", e)),
        }
    }

    // Install accelerate if needed
    if !deps.accelerate.installed {
        match install_dependency("accelerate".to_string()).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("accelerate: {}", e)),
        }
    }

    if !errors.is_empty() {
        Err(format!("Some dependencies failed to install:\n{}\n\nSuccessfully installed:\n{}", 
            errors.join("\n"), 
            results.join("\n")))
    } else if results.is_empty() {
        Ok("All dependencies are already installed!".to_string())
    } else {
        Ok(format!("Successfully installed all missing dependencies:\n{}", results.join("\n")))
    }
}

#[tauri::command]
pub async fn save_hf_token(token: String) -> Result<String, String> {
    use crate::keychain::Keychain;
    
    let keychain = Keychain::new();
    keychain.store("panther", "hf_token", &token)
        .map_err(|e| format!("Failed to save Hugging Face token: {}", e))?;
    
    Ok("Hugging Face token saved successfully".to_string())
}

#[tauri::command]
pub async fn get_hf_token() -> Result<Option<String>, String> {
    use crate::keychain::Keychain;
    
    let keychain = Keychain::new();
    match keychain.retrieve("panther", "hf_token") {
        Ok(token) => Ok(Some(token)),
        Err(_) => Ok(None), // Token doesn't exist yet
    }
}

#[tauri::command]
pub async fn delete_hf_token() -> Result<String, String> {
    use crate::keychain::Keychain;
    
    let keychain = Keychain::new();
    keychain.delete("panther", "hf_token")
        .map_err(|e| format!("Failed to delete Hugging Face token: {}", e))?;
    
    Ok("Hugging Face token deleted successfully".to_string())
}

#[tauri::command]
pub async fn upgrade_dependency(dependency_name: String) -> Result<String, String> {
    // Use the same Python detection logic as install_dependency
    let (python_cmd, python_args) = {
        #[cfg(windows)]
        {
            if Command::new("py").arg("-3.11").arg("--version").output().is_ok() {
                ("py", Some("-3.11"))
            } else if Command::new("py").arg("-3.12").arg("--version").output().is_ok() {
                ("py", Some("-3.12"))
            } else if Command::new("py").arg("--version").output().is_ok() {
                ("py", Some("-3"))
            } else if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Err("Python is not installed.".to_string());
            }
        }
        #[cfg(not(windows))]
        {
            if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Err("Python is not installed.".to_string());
            }
        }
    };

    // Helper to run pip commands
    let run_pip = |args: &[&str]| -> std::io::Result<std::process::Output> {
        if python_cmd == "py" {
            let mut cmd = Command::new("py");
            if let Some(version_arg) = python_args {
                cmd.arg(version_arg);
            }
            cmd.arg("-m").arg("pip");
            for arg in args {
                cmd.arg(arg);
            }
            cmd.output()
        } else {
            let pip_cmd = if python_cmd == "python3" { "pip3" } else { "pip" };
            let mut cmd = Command::new(pip_cmd);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.output()
        }
    };

    // Map dependency name to package name
    let package_name = match dependency_name.as_str() {
        "transformers" => "transformers",
        "datasets" => "datasets",
        "torch" => "torch",
        "accelerate" => "accelerate",
        "huggingface_hub" => "huggingface_hub",
        "hf_xet" => "huggingface_hub[hf_xet]",
        "psutil" => "psutil",
        "safetensors" => "safetensors",
        _ => return Err(format!("Unknown dependency: {}", dependency_name)),
    };

    // Execute pip install --upgrade
    let output = run_pip(&["install", "--upgrade", package_name])
        .map_err(|e| format!("Failed to execute pip upgrade: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(format!("{} upgraded successfully. {}", dependency_name, stdout))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to upgrade {}: {}", dependency_name, stderr))
    }
}

#[tauri::command]
pub async fn uninstall_dependency(dependency_name: String) -> Result<String, String> {
    // Use the same Python detection logic as install_dependency
    let (python_cmd, python_args) = {
        #[cfg(windows)]
        {
            if Command::new("py").arg("-3.11").arg("--version").output().is_ok() {
                ("py", Some("-3.11"))
            } else if Command::new("py").arg("-3.12").arg("--version").output().is_ok() {
                ("py", Some("-3.12"))
            } else if Command::new("py").arg("--version").output().is_ok() {
                ("py", Some("-3"))
            } else if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Err("Python is not installed.".to_string());
            }
        }
        #[cfg(not(windows))]
        {
            if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Err("Python is not installed.".to_string());
            }
        }
    };

    // Helper to run pip commands
    let run_pip = |args: &[&str]| -> std::io::Result<std::process::Output> {
        if python_cmd == "py" {
            let mut cmd = Command::new("py");
            if let Some(version_arg) = python_args {
                cmd.arg(version_arg);
            }
            cmd.arg("-m").arg("pip");
            for arg in args {
                cmd.arg(arg);
            }
            cmd.output()
        } else {
            let pip_cmd = if python_cmd == "python3" { "pip3" } else { "pip" };
            let mut cmd = Command::new(pip_cmd);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.output()
        }
    };

    // Map dependency name to package name
    let package_name = match dependency_name.as_str() {
        "transformers" => "transformers",
        "datasets" => "datasets",
        "torch" => "torch",
        "accelerate" => "accelerate",
        "huggingface_hub" => "huggingface_hub",
        "hf_xet" => "hf_xet", // Uninstall just hf_xet, not the whole huggingface_hub
        "psutil" => "psutil",
        "safetensors" => "safetensors",
        _ => return Err(format!("Unknown dependency: {}", dependency_name)),
    };

    // Execute pip uninstall
    let output = run_pip(&["uninstall", "-y", package_name])
        .map_err(|e| format!("Failed to execute pip uninstall: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(format!("{} uninstalled successfully. {}", dependency_name, stdout))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to uninstall {}: {}", dependency_name, stderr))
    }
}

/// Training environment readiness check with actionable suggestions
#[derive(Debug, Serialize, Deserialize)]
pub struct TrainingReadiness {
    pub ready: bool,
    pub python_ok: bool,
    pub python_version: Option<String>,
    pub python_path: Option<String>,
    pub core_packages_ok: bool,
    pub lora_packages_ok: bool,
    pub gpu_available: bool,
    pub gpu_name: Option<String>,
    pub gpu_memory_gb: Option<f32>,
    pub estimated_max_model_size: Option<String>,
    pub missing_packages: Vec<String>,
    pub warnings: Vec<String>,
    pub recommended_fixes: Vec<String>,
    pub install_all_command: Option<String>,
}

#[tauri::command]
pub async fn check_training_readiness() -> Result<TrainingReadiness, String> {
    let mut readiness = TrainingReadiness {
        ready: false,
        python_ok: false,
        python_version: None,
        python_path: None,
        core_packages_ok: false,
        lora_packages_ok: false,
        gpu_available: false,
        gpu_name: None,
        gpu_memory_gb: None,
        estimated_max_model_size: None,
        missing_packages: Vec::new(),
        warnings: Vec::new(),
        recommended_fixes: Vec::new(),
        install_all_command: None,
    };

    // Detect Python
    let (python_cmd, python_args) = {
        #[cfg(windows)]
        {
            if Command::new("py").arg("-3.11").arg("--version").output().is_ok() {
                ("py", Some("-3.11"))
            } else if Command::new("py").arg("-3.12").arg("--version").output().is_ok() {
                ("py", Some("-3.12"))
            } else if Command::new("py").arg("--version").output().is_ok() {
                ("py", Some("-3"))
            } else if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                readiness.recommended_fixes.push("Install Python 3.11 or 3.12 from python.org".to_string());
                return Ok(readiness);
            }
        }
        #[cfg(not(windows))]
        {
            if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                readiness.recommended_fixes.push("Install Python 3.11 or 3.12".to_string());
                return Ok(readiness);
            }
        }
    };

    readiness.python_ok = true;

    // Helper to run Python commands
    let run_python = |args: &[&str]| -> std::io::Result<std::process::Output> {
        let mut cmd = Command::new(python_cmd);
        if let Some(version_arg) = python_args {
            cmd.arg(version_arg);
        }
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output()
    };

    // Get Python version and path
    if let Ok(output) = run_python(&["--version"]) {
        readiness.python_version = String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string());
    }
    
    if let Ok(output) = run_python(&["-c", "import sys; print(sys.executable)"]) {
        readiness.python_path = String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string());
    }

    // Check if Python 3.13 (CUDA wheels may not exist)
    if let Some(ref v) = readiness.python_version {
        if v.contains("3.13") {
            readiness.warnings.push("Python 3.13 detected. PyTorch CUDA wheels may not be available yet. Consider using Python 3.11 or 3.12 for GPU training.".to_string());
        }
    }

    // Check core packages
    let check_script = r#"
import json
import sys

packages = {}

# Core packages
for pkg in ['transformers', 'datasets', 'torch', 'accelerate']:
    try:
        mod = __import__(pkg)
        packages[pkg] = getattr(mod, '__version__', 'unknown')
    except ImportError:
        packages[pkg] = None

# LoRA packages
for pkg in ['peft', 'trl', 'bitsandbytes']:
    try:
        mod = __import__(pkg)
        packages[pkg] = getattr(mod, '__version__', 'unknown')
    except ImportError:
        packages[pkg] = None

# GPU info
gpu_info = {'available': False, 'name': None, 'memory_gb': None}
try:
    import torch
    if torch.cuda.is_available():
        gpu_info['available'] = True
        gpu_info['name'] = torch.cuda.get_device_name(0)
        gpu_info['memory_gb'] = torch.cuda.get_device_properties(0).total_memory / (1024**3)
except:
    pass

print(json.dumps({'packages': packages, 'gpu': gpu_info}))
"#;

    if let Ok(output) = run_python(&["-c", check_script]) {
        if output.status.success() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&stdout.trim()) {
                    // Parse packages
                    if let Some(packages) = result.get("packages").and_then(|p| p.as_object()) {
                        let core = ["transformers", "datasets", "torch", "accelerate"];
                        let lora = ["peft", "trl", "bitsandbytes"];
                        
                        let mut core_ok = true;
                        let mut lora_ok = true;
                        
                        for pkg in &core {
                            if packages.get(*pkg).and_then(|v| v.as_str()).is_none() {
                                readiness.missing_packages.push(pkg.to_string());
                                core_ok = false;
                            }
                        }
                        
                        for pkg in &lora {
                            if packages.get(*pkg).and_then(|v| v.as_str()).is_none() {
                                readiness.missing_packages.push(pkg.to_string());
                                lora_ok = false;
                            }
                        }
                        
                        readiness.core_packages_ok = core_ok;
                        readiness.lora_packages_ok = lora_ok;
                    }
                    
                    // Parse GPU info
                    if let Some(gpu) = result.get("gpu") {
                        readiness.gpu_available = gpu.get("available").and_then(|v| v.as_bool()).unwrap_or(false);
                        readiness.gpu_name = gpu.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                        readiness.gpu_memory_gb = gpu.get("memory_gb").and_then(|v| v.as_f64()).map(|f| f as f32);
                        
                        // Estimate max model size based on GPU memory
                        if let Some(mem) = readiness.gpu_memory_gb {
                            readiness.estimated_max_model_size = Some(if mem >= 24.0 {
                                "Up to 13B parameters (full) or 70B (QLoRA 4-bit)".to_string()
                            } else if mem >= 16.0 {
                                "Up to 7B parameters (full) or 30B (QLoRA 4-bit)".to_string()
                            } else if mem >= 8.0 {
                                "Up to 3B parameters (full) or 13B (QLoRA 4-bit)".to_string()
                            } else if mem >= 4.0 {
                                "Up to 1B parameters (full) or 7B (QLoRA 4-bit)".to_string()
                            } else {
                                "Limited GPU memory. Consider using QLoRA 4-bit quantization.".to_string()
                            });
                        }
                    }
                }
            }
        }
    }

    // Generate recommended fixes
    let pip_cmd = if python_cmd == "py" {
        if let Some(v) = python_args {
            format!("py {} -m pip", v)
        } else {
            "py -3 -m pip".to_string()
        }
    } else {
        "pip".to_string()
    };

    if !readiness.missing_packages.is_empty() {
        let missing = readiness.missing_packages.join(" ");
        readiness.install_all_command = Some(format!("{} install {}", pip_cmd, missing));
        readiness.recommended_fixes.push(format!("Install missing packages: {} install {}", pip_cmd, missing));
    }

    if !readiness.gpu_available && readiness.core_packages_ok {
        readiness.warnings.push("No GPU detected. Training will use CPU (slower). For GPU support, install PyTorch with CUDA.".to_string());
        #[cfg(windows)]
        {
            readiness.recommended_fixes.push(format!(
                "For GPU support: {} uninstall torch && {} install torch --index-url https://download.pytorch.org/whl/cu121",
                pip_cmd, pip_cmd
            ));
        }
    }

    // bitsandbytes warning for Windows
    #[cfg(windows)]
    {
        if readiness.missing_packages.contains(&"bitsandbytes".to_string()) {
            readiness.warnings.push("bitsandbytes on Windows requires specific CUDA versions. If installation fails, QLoRA will fall back to 8-bit or full precision.".to_string());
        }
    }

    // Set overall readiness
    readiness.ready = readiness.python_ok && readiness.core_packages_ok && readiness.lora_packages_ok;

    Ok(readiness)
}
