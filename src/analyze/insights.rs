//! Per-project-type overview and recommendations written into `project.json`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiInsights {
    pub overview: String,
    pub recommendations: Vec<String>,
}

/// The overview and recommendations for the primary project type. A project with fewer than
/// ten files also gets one extra recommendation.
#[must_use]
pub fn generate_ai_insights(project_type: &str, total_files: usize) -> AiInsights {
    let (overview, recs) = table(project_type);
    let mut recommendations: Vec<String> = recs.iter().map(|r| (*r).to_owned()).collect();
    if total_files < 10 {
        recommendations.push("Expand project with additional features and modules".to_owned());
    }
    AiInsights {
        overview: overview.to_owned(),
        recommendations,
    }
}

#[allow(clippy::too_many_lines)]
fn table(project_type: &str) -> (&'static str, &'static [&'static str]) {
    match project_type {
        "nextjs" => (
            "Next.js application with React framework featuring SSR/SSG capabilities",
            &[
                "Implement comprehensive SEO optimization with metadata API",
                "Add performance monitoring with Next.js Analytics",
                "Consider incremental static regeneration for dynamic content",
            ],
        ),
        "react" => (
            "React application with component-based architecture and hooks system",
            &[
                "Consider adding TypeScript for better type safety",
                "Implement comprehensive unit tests with Jest and React Testing Library",
                "Add state management solution like Redux Toolkit or Zustand",
            ],
        ),
        "vue" => (
            "Vue.js application with reactive component system and progressive framework",
            &[
                "Consider Vue 3 Composition API for better code organization",
                "Add TypeScript support with vue-tsc",
                "Implement Vue Router for single page applications",
            ],
        ),
        "nuxt" => (
            "Nuxt.js application providing Vue.js framework with SSR capabilities",
            &[
                "Optimize for search engines with proper meta tags",
                "Implement static site generation for better performance",
                "Add Nuxt modules for enhanced functionality",
            ],
        ),
        "svelte" => (
            "Svelte application with compile-time optimizations and reactive components",
            &[
                "Add TypeScript support for better development experience",
                "Implement SvelteKit for full-stack applications",
                "Consider adding unit tests with Vitest",
            ],
        ),
        "astro" => (
            "Astro application focused on content-rich sites with minimal JavaScript",
            &[
                "Optimize for performance with island architecture",
                "Implement proper SEO and meta management",
                "Add Markdown/MDX content processing capabilities",
            ],
        ),
        "remix" => (
            "Remix web framework with focus on web fundamentals and nested routes",
            &[
                "Implement proper error boundaries and loading states",
                "Add database integration with Prisma or similar",
                "Optimize for progressive enhancement",
            ],
        ),
        "gatsby" => (
            "Gatsby static site generator with React and GraphQL data layer",
            &[
                "Optimize build performance with Gatsby caching",
                "Implement proper image optimization with gatsby-plugin-image",
                "Add SEO optimization with gatsby-plugin-react-helmet",
            ],
        ),
        "express" => (
            "Express.js web application with minimal Node.js framework",
            &[
                "Add comprehensive error handling middleware",
                "Implement proper logging with Winston or Morgan",
                "Consider adding TypeScript support",
            ],
        ),
        "nestjs" => (
            "NestJS application with TypeScript and architectural patterns",
            &[
                "Implement comprehensive validation with class-validator",
                "Add OpenAPI/Swagger documentation",
                "Consider microservices architecture with NestJS modules",
            ],
        ),
        "vite" => (
            "Vite-powered development environment with fast build tooling",
            &[
                "Configure proper build optimization for production",
                "Add plugin ecosystem for enhanced functionality",
                "Implement HMR optimization for development workflow",
            ],
        ),
        "django" => (
            "Django web application with batteries-included framework",
            &[
                "Add comprehensive API documentation with Django REST framework",
                "Implement proper caching strategies with Redis",
                "Add background task processing with Celery",
            ],
        ),
        "fastapi" => (
            "FastAPI application with high-performance async web framework",
            &[
                "Add comprehensive OpenAPI/Swagger documentation",
                "Implement dependency injection for better testing",
                "Add async database operations with SQLAlchemy 2.0",
            ],
        ),
        "flask" => (
            "Flask web application with lightweight and extensible framework",
            &[
                "Add SQLAlchemy for database management",
                "Implement proper application factory pattern",
                "Add Flask extensions for enhanced functionality",
            ],
        ),
        "poetry" => (
            "Python project managed with Poetry for dependency management",
            &[
                "Configure proper dependency groups for development",
                "Add comprehensive pytest configuration",
                "Implement pre-commit hooks with poetry hooks",
            ],
        ),
        "pipenv" => (
            "Python project with Pipenv for virtual environment and dependency management",
            &[
                "Configure proper Pipfile for development dependencies",
                "Add pipenv scripts for common tasks",
            ],
        ),
        "flutter" => (
            "Flutter application with cross-platform mobile development framework",
            &[
                "Add comprehensive unit tests with flutter test",
                "Add effective linter configuration with flutter analyze",
                "Consider adding integration tests for critical user flows",
            ],
        ),
        "gin" => (
            "Gin web framework for Go with high-performance HTTP router",
            &[
                "Add comprehensive middleware for authentication and logging",
                "Implement proper error handling and validation",
                "Add database integration with GORM or sqlx",
            ],
        ),
        "echo" => (
            "Echo web framework for Go with high performance and extensibility",
            &[
                "Implement proper middleware stack",
                "Add comprehensive error handling",
                "Consider adding database integration",
            ],
        ),
        "fiber" => (
            "Fiber web framework for Go inspired by Express.js",
            &[
                "Add comprehensive middleware configuration",
                "Implement proper error handling",
                "Add database integration for full-stack applications",
            ],
        ),
        "actix" => (
            "Actix Web framework for Rust with high-performance actor model",
            &[
                "Implement proper middleware for authentication",
                "Add comprehensive error handling",
                "Consider adding database integration with sqlx",
            ],
        ),
        "rocket" => (
            "Rocket web framework for Rust with type-safe and fast development",
            &[
                "Implement proper database integration with Diesel",
                "Add comprehensive testing strategies",
                "Configure proper async/await patterns",
            ],
        ),
        "axum" => (
            "Axum web framework for Rust built on Tokio with async support",
            &[
                "Implement proper async database integration",
                "Add comprehensive middleware for logging and tracing",
                "Consider adding tower middleware ecosystem",
            ],
        ),
        "rust" => (
            "Rust application with focus on memory safety and performance",
            &[
                "Add comprehensive unit and integration tests",
                "Implement proper error handling with Result types",
                "Consider adding async runtime optimization",
            ],
        ),
        "springboot" => (
            "Spring Boot application with enterprise-grade framework",
            &[
                "Add comprehensive Spring Security configuration",
                "Implement proper database integration with Spring Data JPA",
                "Add RESTful API documentation with OpenAPI",
            ],
        ),
        "spring" => (
            "Spring framework application with dependency injection and AOP",
            &[
                "Configure proper Spring context management",
                "Add comprehensive testing with Spring Test",
                "Implement proper MVC patterns",
            ],
        ),
        "maven" => (
            "Java project managed with Maven build system",
            &[
                "Configure proper dependency management",
                "Add comprehensive plugin configuration",
                "Implement proper build lifecycle",
            ],
        ),
        "gradle" => (
            "Java project managed with Gradle build system",
            &[
                "Configure proper build scripts with Kotlin DSL",
                "Add comprehensive plugin ecosystem",
                "Implement proper dependency management",
            ],
        ),
        "ktor" => (
            "Ktor framework for Kotlin with asynchronous web applications",
            &[
                "Add comprehensive structured logging",
                "Implement proper content negotiation",
                "Add database integration with Exposed",
            ],
        ),
        "rails" => (
            "Ruby on Rails application with convention over configuration",
            &[
                "Add comprehensive testing with RSpec or Minitest",
                "Implement proper background jobs with Sidekiq",
                "Add API versioning strategies",
            ],
        ),
        "sinatra" => (
            "Sinatra DSL for Ruby with lightweight web applications",
            &[
                "Add comprehensive middleware configuration",
                "Implement proper RESTful API patterns",
                "Consider adding ActiveRecord for database",
            ],
        ),
        "laravel" => (
            "Laravel PHP framework with elegant syntax and comprehensive ecosystem",
            &[
                "Add comprehensive API documentation with Laravel API resources",
                "Implement proper queue system with Redis and Horizon",
                "Add comprehensive testing with PHPUnit and Laravel Dusk",
            ],
        ),
        "symfony" => (
            "Symfony PHP framework with reusable components and MVC architecture",
            &[
                "Configure proper service container and dependency injection",
                "Add comprehensive form validation and security",
                "Implement proper API platform integration",
            ],
        ),
        "wordpress" => (
            "WordPress content management system with plugin and theme development",
            &[
                "Implement proper WordPress coding standards",
                "Add comprehensive security practices",
                "Consider adding custom post types and taxonomies",
            ],
        ),
        "aspnet" => (
            "ASP.NET Core application with cross-platform web framework",
            &[
                "Add comprehensive authentication and authorization",
                "Implement proper dependency injection patterns",
                "Add Entity Framework Core for database operations",
            ],
        ),
        "dotnet" => (
            ".NET application with managed runtime and comprehensive framework",
            &[
                "Configure proper NuGet package management",
                "Add comprehensive unit testing with xUnit or NUnit",
                "Implement proper async/await patterns",
            ],
        ),
        "swift" => (
            "Swift application with safe and performant programming language",
            &[
                "Add comprehensive unit tests with XCTest",
                "Implement proper error handling with Result types",
                "Consider adding async/await patterns",
            ],
        ),
        "docker" => (
            "Docker containerized application with containerization platform",
            &[
                "Add multi-stage builds for optimization",
                "Implement proper health checks",
                "Consider adding Docker Compose for orchestration",
            ],
        ),
        "docker-compose" => (
            "Docker Compose application with multi-container orchestration",
            &[
                "Configure proper networking and volumes",
                "Add environment-specific configurations",
                "Implement proper service dependencies",
            ],
        ),
        "terraform" => (
            "Terraform infrastructure as code with cloud resource management",
            &[
                "Implement proper state management strategies",
                "Add comprehensive variable validation",
                "Configure proper remote state backends",
            ],
        ),
        "vagrant" => (
            "Vagrant development environment with virtualization management",
            &[
                "Configure proper provisioning scripts",
                "Add multi-machine setup if needed",
                "Implement proper network configuration",
            ],
        ),
        "kustomize" => (
            "Kustomize Kubernetes configuration management with template-free customization",
            &[
                "Configure proper overlay structure",
                "Add comprehensive resource organization",
                "Implement proper secret management",
            ],
        ),
        _ => (
            "Generic project structure",
            &[
                "Add README.md with project documentation",
                "Consider adding automated testing",
                "Implement proper version control practices",
            ],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_project_gets_the_extra_recommendation() {
        let small = generate_ai_insights("rust", 3);
        let large = generate_ai_insights("rust", 30);
        assert_eq!(small.recommendations.len(), large.recommendations.len() + 1);
    }

    #[test]
    fn an_unknown_type_gets_the_generic_overview() {
        assert_eq!(
            generate_ai_insights("nope", 30).overview,
            "Generic project structure"
        );
    }
}
