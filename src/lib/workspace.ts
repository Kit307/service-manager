import type { Project, ServiceDefinition, ServiceRef } from "./types";
export type ServiceEntry = { project: Project; service: ServiceDefinition };
export const refKey = (ref: ServiceRef) =>
  JSON.stringify([ref.projectId, ref.serviceId]);
export const serviceRef = ({ project, service }: ServiceEntry): ServiceRef => ({
  projectId: project.id,
  serviceId: service.id,
});
export const entries = (projects: Project[]): ServiceEntry[] =>
  projects.flatMap((project) =>
    project.config.services.map((service) => ({ project, service })),
  );
